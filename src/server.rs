#![allow(dead_code)]

use std::{
    collections::HashMap,
    io::ErrorKind,
    net::Ipv4Addr,
    ops::Deref,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use num_traits::FromPrimitive;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{debug, error, info};

use crate::{
    driver::{Driver, ImageDesc},
    proto::{
        self, NbdClientFlag, NbdCmd, NbdHandshakeFlag, NbdOpt, NbdOptReply, IHAVEOPT, INIT_PASSWD,
        NBD_REQUEST_MAGIC,
    },
};

pub type IoResult<T> = std::io::Result<T>;

// TODO
const MAX_OPTION_DATA_LEN: usize = 4096;

trait NbdWrite {
    async fn nbd_write(&self, sock: &mut TcpStream) -> IoResult<()>;
}

trait NbdRead: Sized {
    async fn nbd_read(sock: &mut TcpStream) -> IoResult<Self>;
}

pub struct ServerBuilder {
    port: u16,
    handshake_flags: u16,
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self {
            port: crate::proto::NBD_NEWSTYLE_PORT,
            handshake_flags: NbdHandshakeFlag::FIXED_NEWSTYLE.bits(),
        }
    }
}

impl ServerBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn port(self, port: u16) -> Self {
        Self { port, ..self }
    }

    pub fn build(self) -> Server {
        Server {
            config: Arc::new(ServerConfig::new(self.port, self.handshake_flags)),
            state: Arc::new(Mutex::new(ServerState::default())),
        }
    }
}

pub struct ServerConfig {
    port: u16,
    handshake_flags: u16,
    option_handlers: HashMap<NbdOpt, Box<dyn OptionHandler>>,
}

impl ServerConfig {
    fn new(port: u16, handshake_flags: u16) -> Self {
        let mut config = ServerConfig {
            port,
            handshake_flags,
            option_handlers: HashMap::new(),
        };
        config.setup_option_handlers();
        config
    }

    fn setup_option_handlers(&mut self) {
        self.option_handlers
            .insert(NbdOpt::List, Box::new(ListOptionHandler::default()));
    }

    async fn handle_option(
        &self,
        server_shard: &mut ServerShard,
        opt: NbdOpt,
        data: Vec<u8>,
        sock: &mut TcpStream,
    ) -> IoResult<OptionHandleState> {
        if let Some(handler) = self.option_handlers.get(&opt) {
            handler.handle_option(server_shard, opt, data, sock).await
        } else {
            UnknownOptionHandler::default()
                .handle_option(server_shard, opt, data, sock)
                .await
        }
    }
}

#[derive(Debug, Default)]
struct ServerState {
    default_driver: Option<Driver>,
    images: HashMap<Driver, Vec<ImageDesc>>,
}

impl ServerState {
    fn list_images(&self) -> Vec<(Driver, ImageDesc)> {
        self.images
            .iter()
            .flat_map(|(k, v)| v.iter().map(|image| (k.clone(), image.clone())))
            .collect()
    }

    fn list_images_full_name(&self) -> Vec<String> {
        self.list_images()
            .into_iter()
            .map(|(_, image)| image.full_name())
            .collect()
    }
}

pub struct Server {
    config: Arc<ServerConfig>,
    state: Arc<Mutex<ServerState>>,
}

impl Server {
    pub async fn run(&mut self) -> IoResult<()> {
        // Listen for client connection.
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, self.config.port)).await?;
        loop {
            let (sock, addr) = listener.accept().await?;
            info!(?addr, "accept new connection");
            let shard = ServerShard {
                config: self.config.clone(),
                state: self.state.clone(),
            };
            tokio::spawn(shard.handle_connection(sock));
        }
    }
}

struct ServerShard {
    config: Arc<ServerConfig>,
    state: Arc<Mutex<ServerState>>,
}

impl ServerShard {
    async fn handle_connection(mut self, mut sock: TcpStream) -> IoResult<()> {
        // Handshake.
        sock.write_u64(INIT_PASSWD).await?;
        sock.write_u64(IHAVEOPT).await?;
        sock.write_u16(self.config.handshake_flags).await?;
        sock.flush().await?;

        let client_flags = NbdClientFlag::from_bits_retain(sock.read_u32().await?);
        info!(?client_flags, "read from client");
        if !client_flags.contains(NbdClientFlag::FIXED_NEWSTYLE) {
            error!("client do not support fixed newstyle negotiation");
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        let config = self.config.clone();

        // Handle options.
        loop {
            let i_have_opt = sock.read_u64().await?;
            if i_have_opt != IHAVEOPT {
                error!(?i_have_opt, "unknown magic number");
                return Err(std::io::ErrorKind::InvalidData.into());
            }
            let option = sock.read_u32().await?;
            let option: NbdOpt = FromPrimitive::from_u32(option).ok_or_else(|| {
                error!(option, "unknown nbd option");
                std::io::Error::from(std::io::ErrorKind::InvalidData)
            })?;
            info!(?option, "handle option");

            let option_data_len = sock.read_u32().await?;
            info!(?option_data_len, "option data len");
            if option_data_len as usize > MAX_OPTION_DATA_LEN {
                error!(option_data_len, "option data is too large");
                return Err(std::io::ErrorKind::InvalidData.into());
            }

            let mut option_data: Vec<u8> = Vec::with_capacity(option_data_len as usize);
            option_data.resize(option_data_len as usize, 0);
            sock.read_exact(&mut option_data).await?;

            match config
                .handle_option(&mut self, option, option_data, &mut sock)
                .await?
            {
                OptionHandleState::Continue => continue,
                OptionHandleState::End => break,
                OptionHandleState::Abort => return Err(std::io::ErrorKind::InvalidData.into()),
            }
        }

        // Transmission.
        loop {
            // Handle request.
            let req = Request::nbd_read(&mut sock).await?;
            let trans_end = self.handle_request(req, &mut sock).await?;
            if trans_end {
                break;
            }
        }
        info!("transmission completed");
        Ok(())
    }

    async fn handle_option(
        &mut self,
        opt: NbdOpt,
        data: Vec<u8>,
        sock: &mut TcpStream,
    ) -> IoResult<bool> {
        unimplemented!()
    }

    async fn handle_request(&mut self, req: Request, sock: &mut TcpStream) -> IoResult<bool> {
        unimplemented!()
    }
}

struct Request {
    flags: u16,
    cmd: NbdCmd,
    cookie: u64,
    offset: u64,
    length: u32,
    data: Vec<u8>,
}

impl NbdRead for Request {
    async fn nbd_read(sock: &mut TcpStream) -> IoResult<Self> {
        let request_magic = sock.read_u32().await?;
        if request_magic != NBD_REQUEST_MAGIC {
            error!(?request_magic, "request magic mismatch");
            return Err(std::io::ErrorKind::InvalidData.into());
        }

        let flags = sock.read_u16().await?;
        let cmd = sock.read_u16().await?;
        let cookie = sock.read_u64().await?;
        let offset = sock.read_u64().await?;
        let length = sock.read_u32().await?;
        debug!(flags, cmd, cookie, offset, length, "read request");

        let cmd: NbdCmd =
            FromPrimitive::from_u16(cmd).ok_or(std::io::Error::from(ErrorKind::InvalidData))?;
        let mut data: Vec<u8> = Vec::new();

        if cmd == NbdCmd::Write {
            data.reserve(length as usize);
            unsafe { data.set_len(length as usize) };
            sock.read_exact(&mut data).await?;
        }

        Ok(Request {
            flags,
            cmd,
            cookie,
            offset,
            length,
            data,
        })
    }
}

struct OptReply {
    option: NbdOpt,
    reply: NbdOptReply,
    data: Vec<u8>,
}

impl NbdWrite for OptReply {
    async fn nbd_write(&self, sock: &mut TcpStream) -> IoResult<()> {
        sock.write_u64(proto::NBD_OPT_REPLY_MAGIC).await?;
        sock.write_u32(self.option as u32).await?;
        sock.write_i32(self.reply as i32).await?;
        sock.write_u32(self.data.len().try_into().unwrap()).await?;
        if self.data.len() != 0 {
            sock.write_all(&self.data).await?;
        }
        Ok(())
    }
}

enum OptionHandleState {
    Continue,
    End,
    Abort,
}

#[async_trait]
trait OptionHandler: Send + Sync {
    async fn handle_option(
        &self,
        server_shard: &mut ServerShard,
        opt: NbdOpt,
        data: Vec<u8>,
        sock: &mut TcpStream,
    ) -> IoResult<OptionHandleState>;
}

#[derive(Debug, Default)]
struct UnknownOptionHandler {}

#[async_trait]
impl OptionHandler for UnknownOptionHandler {
    async fn handle_option(
        &self,
        _server_shard: &mut ServerShard,
        opt: NbdOpt,
        _data: Vec<u8>,
        sock: &mut TcpStream,
    ) -> IoResult<OptionHandleState> {
        let reply = OptReply {
            option: opt,
            reply: NbdOptReply::ErrUnsup,
            data: format!("unknown option {}", opt as i32).into_bytes(),
        };
        reply.nbd_write(sock).await?;
        Ok(OptionHandleState::Continue)
    }
}

#[derive(Debug, Default)]
struct ListOptionHandler {}

#[async_trait]
impl OptionHandler for ListOptionHandler {
    async fn handle_option(
        &self,
        server_shard: &mut ServerShard,
        opt: NbdOpt,
        _data: Vec<u8>,
        sock: &mut TcpStream,
    ) -> IoResult<OptionHandleState> {
        let images = server_shard.state.lock().unwrap().list_images_full_name();
        for image in images {
            let mut data = BytesMut::new();
            data.put_u32_ne(image.as_bytes().len() as u32);
            data.put_slice(image.as_bytes());
            data.put_slice(&[0, 0, 0, 0]);
            let reply = OptReply {
                option: opt,
                reply: NbdOptReply::Server,
                data: Vec::from(data.deref()),
            };
            reply.nbd_write(sock).await?;
        }
        OptReply {
            option: opt,
            reply: NbdOptReply::Ack,
            data: Vec::new(),
        }
        .nbd_write(sock)
        .await?;
        Ok(OptionHandleState::Continue)
    }
}
