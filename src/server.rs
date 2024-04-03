#![allow(dead_code)]

pub type IoResult<T> = std::io::Result<T>;

pub struct ServerBuilder {
    port: u16,
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self {
            port: crate::proto::NBD_NEWSTYLE_PORT,
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
        Server { port: self.port }
    }
}

pub struct Server {
    port: u16,
}

impl Server {
    pub async fn run(&mut self) -> IoResult<()> {
        unimplemented!()
    }
}
