#![allow(unused_variables)]

use async_trait::async_trait;

use crate::utils::IoResult;

use super::{DriverConstructor, DriverImpl, DriverRegistry, Image, ImageDesc};

pub struct FsDriver {}

#[async_trait]
impl DriverImpl for FsDriver {
    fn name(&self) -> &str {
        "fs"
    }

    fn dup(&self) -> Box<dyn DriverImpl> {
        Box::new(FsDriver {})
    }

    async fn get_image(&self, name: &str) -> IoResult<ImageDesc> {
        unimplemented!()
    }

    async fn open(&self, image: &ImageDesc) -> IoResult<Image> {
        unimplemented!()
    }
}

struct FsDriverConstructor {}

impl DriverConstructor for FsDriverConstructor {
    fn name(&self) -> String {
        format!("fs")
    }

    fn construct(&self, _config: &super::DriverConfig) -> Box<dyn DriverImpl> {
        Box::new(FsDriver {})
    }
}

pub fn init_driver(registry: &mut DriverRegistry) {
    registry.register_driver(FsDriverConstructor {})
}
