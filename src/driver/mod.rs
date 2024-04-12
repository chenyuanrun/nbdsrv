#![allow(dead_code)]

pub mod fs;

use std::{collections::HashMap, fmt::Debug, ops::Deref, str::FromStr, sync::OnceLock};

use async_trait::async_trait;

use crate::utils::IoResult;

// Driver registry

pub struct DriverConfig {
    config: HashMap<String, String>,
}

pub trait DriverConstructor: Send + Sync + 'static {
    fn name(&self) -> String;
    fn construct(&self, config: &DriverConfig) -> Box<dyn DriverImpl>;
}

pub struct DriverRegistry {
    driver_constructors: Vec<Box<dyn DriverConstructor>>,
}

impl DriverRegistry {
    fn new() -> Self {
        let mut registry = DriverRegistry {
            driver_constructors: Vec::new(),
        };

        fs::init_driver(&mut registry);

        registry
    }

    pub fn register_driver<T: DriverConstructor>(&mut self, constructor: T) {
        self.driver_constructors.push(Box::new(constructor))
    }

    pub fn list_drivers(&self) -> Vec<String> {
        self.driver_constructors
            .iter()
            .map(|item| item.name())
            .collect()
    }

    pub fn get_driver(&self, name: &str, config: &DriverConfig) -> Option<Driver> {
        self.driver_constructors
            .iter()
            .find(|item| &item.name() == name)
            .map(|constructor| Driver::from_impl(constructor.construct(config)))
    }
}

static DRIVER_REGISTRY: OnceLock<DriverRegistry> = OnceLock::new();

pub fn driver_registry() -> &'static DriverRegistry {
    DRIVER_REGISTRY.get_or_init(|| DriverRegistry::new())
}

pub struct Driver {
    driver_impl: Box<dyn DriverImpl>,
}

impl std::hash::Hash for Driver {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name().hash(state);
    }
}

impl PartialEq for Driver {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl Eq for Driver {}

impl Clone for Driver {
    fn clone(&self) -> Self {
        Driver {
            driver_impl: self.driver_impl.dup(),
        }
    }
}

impl Debug for Driver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Driver")
            .field("name", &self.name())
            .finish()
    }
}

impl Driver {
    pub fn from_impl(driver_impl: Box<dyn DriverImpl>) -> Self {
        Self { driver_impl }
    }
}

impl Deref for Driver {
    type Target = dyn DriverImpl;
    fn deref(&self) -> &Self::Target {
        self.driver_impl.as_ref()
    }
}

#[async_trait]
pub trait DriverImpl: Send + Sync {
    fn name(&self) -> &str;
    fn dup(&self) -> Box<dyn DriverImpl>;
    async fn get_image(&self, name: &str) -> IoResult<ImageDesc>;
    async fn open(&self, image: &ImageDesc) -> IoResult<Image>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageDesc {
    pub driver_name: String,
    pub name: String,
}

impl ImageDesc {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.name, self.driver_name)
    }
}

impl FromStr for ImageDesc {
    type Err = std::io::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((drv, image)) = s.split_once('/') {
            if drv.is_empty() || image.is_empty() {
                Err(std::io::ErrorKind::InvalidData.into())
            } else {
                Ok(ImageDesc {
                    driver_name: drv.to_string(),
                    name: image.to_string(),
                })
            }
        } else {
            Err(std::io::ErrorKind::InvalidData.into())
        }
    }
}

pub struct Image {
    blkdev_impl: Box<dyn ImageImpl>,
}

impl Clone for Image {
    fn clone(&self) -> Self {
        Self {
            blkdev_impl: self.blkdev_impl.dup(),
        }
    }
}

impl Deref for Image {
    type Target = dyn ImageImpl;
    fn deref(&self) -> &Self::Target {
        self.blkdev_impl.as_ref()
    }
}

impl Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image").field("name", &self.name()).finish()
    }
}

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub size: usize,
    pub readonly: bool,
}

#[async_trait]
pub trait ImageImpl: Send + Sync {
    fn name(&self) -> &str;
    fn info(&self) -> ImageInfo;
    fn dup(&self) -> Box<dyn ImageImpl>;
}
