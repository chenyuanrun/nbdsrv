use async_trait::async_trait;

#[derive(Clone)]
pub struct DriverRegistry {}

impl DriverRegistry {
    pub fn new() -> Self {
        DriverRegistry {}
    }

    pub fn get_driver(&self, name: &str) -> Option<()> {
        unimplemented!()
    }
}

#[derive(Clone)]
pub struct Driver {}

impl Driver {
    pub fn name(&self) -> &str {
        unimplemented!()
    }
}

#[async_trait]
pub trait DriverImpl: Send + Sync {
    fn name(&self) -> &str;
    fn dup(&self) -> Box<dyn DriverImpl>;
}

pub struct FsDriver {}
