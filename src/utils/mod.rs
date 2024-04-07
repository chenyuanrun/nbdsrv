pub mod alloc;
pub mod linked_list;

pub trait Transmute<T> {
    unsafe fn transmute(self) -> T;
}

impl<'a, T> Transmute<&'a T> for *const T {
    unsafe fn transmute(self) -> &'a T {
        &*self
    }
}

impl<'a, T> Transmute<&'a mut T> for *mut T {
    unsafe fn transmute(self) -> &'a mut T {
        &mut *self
    }
}
