use std::{cell::UnsafeCell, ptr::addr_of_mut};

use crate::{
    container_of, list_for_each_entry,
    utils::{alloc::malloc, linked_list::ListHead, Transmute},
};
use async_trait::async_trait;
use libc::{pthread_mutex_lock, pthread_mutex_unlock};

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

fn fs_driver_constructor() -> Box<dyn DriverImpl> {
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

type DriverConstructor = fn() -> Box<dyn DriverImpl>;

struct DriverItem {
    list: ListHead,
    name_len: usize,
    name: [u8; 64],
    constructor: Option<DriverConstructor>,
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
