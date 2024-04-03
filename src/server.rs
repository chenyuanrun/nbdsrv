#![allow(dead_code)]

use std::{io::ErrorKind, net::Ipv4Addr, sync::Arc};

use num_traits::FromPrimitive;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{debug, error, info};

use crate::proto::{
    NbdClientFlag, NbdCmd, NbdHandshakeFlag, NbdOpt, IHAVEOPT, INIT_PASSWD, NBD_REQUEST_MAGIC,
};

pub type IoResult<T> = std::io::Result<T>;

// TODO
const MAX_OPTION_DATA_LEN: usize = 4096;

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
            config: Arc::new(ServerConfig {
                port: self.port,
                handshake_flags: self.handshake_flags,
            }),
        }
    }
}

pub struct ServerConfig {
    port: u16,
    handshake_flags: u16,
}

pub struct Server {
    config: Arc<ServerConfig>,
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
            };
            tokio::spawn(shard.handle_connection(sock));
        }
    }
}

struct ServerShard {
    config: Arc<ServerConfig>,
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

            let neg_end = self.handle_option(option, option_data, &mut sock).await?;
            if neg_end {
                break;
            }
        }

        // Transmission.
        loop {
            // Handle request.
            let req = self.read_request(&mut sock).await?;
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

    async fn read_request(&mut self, sock: &mut TcpStream) -> IoResult<Request> {
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
