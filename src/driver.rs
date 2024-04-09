use std::{cell::UnsafeCell, collections::HashMap, fmt::Debug, ops::Deref, ptr::addr_of_mut};

use crate::{
    container_of, list_for_each_entry,
    utils::{alloc::malloc, linked_list::ListHead, IoResult, Transmute},
};
use async_trait::async_trait;
use libc::{pthread_mutex_lock, pthread_mutex_unlock};

#[derive(Clone)]
pub struct DriverRegistry {}

impl DriverRegistry {
    pub fn new() -> Self {
        DriverRegistry {}
    }

    pub fn list_drivers(&self) -> Vec<String> {
        let mut drivers = Vec::new();
        unsafe {
            pthread_mutex_lock(DRIVERS.get_lock());
            list_for_each_entry!(DriverItem, DRIVERS.get_drivers(), list, |item| => {
                drivers.push(item.transmute().get_name().to_string())
            });
            pthread_mutex_unlock(DRIVERS.get_lock());
        }
        drivers
    }

    pub fn get_driver(&self, name: &str, config: &HashMap<String, String>) -> Option<Driver> {
        let mut driver = None;
        unsafe {
            pthread_mutex_lock(DRIVERS.get_lock());
            list_for_each_entry!(DriverItem, DRIVERS.get_drivers(), list, |item| => {
                if item.transmute().get_name() == name {
                    driver = Some(Driver::from_impl(item.transmute().constructor.as_ref().unwrap()(config)));
                    break;
                }
            });
            pthread_mutex_unlock(DRIVERS.get_lock());
        }
        driver
    }
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

#[derive(Debug, Clone)]
pub struct ImageDesc {
    pub name: String,
    pub driver_name: String,
    pub config: HashMap<String, String>,
}

impl ImageDesc {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.name, self.driver_name)
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

#[async_trait]
pub trait ImageImpl: Send + Sync {
    fn name(&self) -> &str;
    fn dup(&self) -> Box<dyn ImageImpl>;
}

pub struct FsDriver {}

fn fs_driver_constructor(config: &HashMap<String, String>) -> Box<dyn DriverImpl> {
    unimplemented!()
}

#[ctor::ctor]
fn _register_fs_driver() {
    register_driver("fs", fs_driver_constructor);
}

// Driver register.

struct GlobalDrivers(UnsafeCell<GlobalDriversInner>);

struct GlobalDriversInner {
    init_once: libc::pthread_once_t,
    lock: libc::pthread_mutex_t,
    drivers: ListHead,
}

unsafe impl Sync for GlobalDrivers {}

impl GlobalDrivers {
    fn get_init_once(&self) -> *mut libc::pthread_once_t {
        unsafe { std::ptr::addr_of_mut!(self.0.get().transmute().init_once) }
    }

    fn get_lock(&self) -> *mut libc::pthread_mutex_t {
        unsafe { std::ptr::addr_of_mut!(self.0.get().transmute().lock) }
    }

    fn get_drivers(&self) -> *mut ListHead {
        unsafe { std::ptr::addr_of_mut!(self.0.get().transmute().drivers) }
    }
}

type DriverConstructor = fn(&HashMap<String, String>) -> Box<dyn DriverImpl>;

struct DriverItem {
    list: ListHead,
    name_len: usize,
    name: [u8; 64],
    constructor: Option<DriverConstructor>,
}

impl DriverItem {
    fn get_name(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.name[..self.name_len]) }
    }
}

static DRIVERS: GlobalDrivers = GlobalDrivers(UnsafeCell::new(GlobalDriversInner {
    init_once: libc::PTHREAD_ONCE_INIT,
    lock: libc::PTHREAD_MUTEX_INITIALIZER,
    drivers: ListHead::null(),
}));

fn init_drivers() {
    unsafe {
        libc::pthread_once(DRIVERS.get_init_once(), init_drivers_impl);
    }
}

extern "C" fn init_drivers_impl() {
    unsafe { ListHead::init(DRIVERS.get_drivers()) };
}

pub fn register_driver(name: &str, constructor: DriverConstructor) {
    init_drivers();
    unsafe {
        pthread_mutex_lock(DRIVERS.get_lock());

        let mut found = false;

        list_for_each_entry!(DriverItem, DRIVERS.get_drivers(), list, |item| => {
            let item_name = std::str::from_utf8_unchecked(&item.transmute().name[..item.transmute().name_len]);
            if item_name == name {
                found = true;
                break;
            }
        });

        if !found {
            let new_driver = malloc::<DriverItem>(std::mem::MaybeUninit::zeroed().assume_init());
            ListHead::init(addr_of_mut!(new_driver.transmute().list));
            if name.as_bytes().len() > 64 {
                pthread_mutex_unlock(DRIVERS.get_lock());
                libc::abort();
            }
            new_driver.transmute().name_len = name.as_bytes().len();
            std::ptr::copy(
                name.as_ptr(),
                new_driver.transmute().name.as_mut_ptr(),
                name.as_bytes().len(),
            );
            new_driver.transmute().constructor = Some(constructor);

            ListHead::add(
                DRIVERS.get_drivers(),
                addr_of_mut!(new_driver.transmute().list),
            );
        }

        pthread_mutex_unlock(DRIVERS.get_lock());
    }
}

// Tests
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_drivers() {
        let registry = DriverRegistry::new();
        let drivers = registry.list_drivers();

        let mut fs_found = false;
        for driver in drivers {
            println!("found driver [{driver}]");
            if &driver == "fs" {
                fs_found = true;
            }
        }
        assert!(fs_found);
    }
}
