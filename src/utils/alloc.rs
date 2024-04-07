pub unsafe fn malloc<T>(value: T) -> *mut T {
    let align = std::mem::align_of::<T>();
    let size = std::mem::size_of::<T>();
    let mut ptr: *mut T = std::ptr::null_mut();
    let res = libc::posix_memalign(
        std::mem::transmute(std::ptr::addr_of_mut!(ptr)),
        align,
        size,
    );

    if res != 0 {
        libc::abort();
    }

    std::ptr::write(ptr, value);
    ptr
}

pub unsafe fn free<T>(ptr: *mut T) {
    libc::free(ptr as *mut _);
}

pub unsafe fn drop_and_free<T>(ptr: *mut T) {
    std::ptr::drop_in_place(ptr);
    free(ptr);
}
