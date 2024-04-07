use crate::utils::Transmute;

pub struct ListHead {
    next: *mut ListHead,
    prev: *mut ListHead,
}

unsafe impl Sync for ListHead {}

impl ListHead {
    pub const fn null() -> Self {
        Self {
            next: std::ptr::null_mut(),
            prev: std::ptr::null_mut(),
        }
    }

    pub unsafe fn init(this: *mut ListHead) {
        this.transmute().next = this;
        this.transmute().prev = this;
    }

    pub unsafe fn iter(this: *mut ListHead) -> ListIter {
        ListIter {
            head: this,
            cur: this.transmute().next,
        }
    }

    pub unsafe fn add(head: *mut ListHead, new: *mut ListHead) {
        new.transmute().next = head.transmute().next;
        new.transmute().prev = head;

        head.transmute().next.transmute().prev = new;
        head.transmute().next = new;
    }

    pub unsafe fn add_tail(head: *mut ListHead, new: *mut ListHead) {
        new.transmute().next = head;
        new.transmute().prev = head.transmute().prev;

        head.transmute().prev.transmute().next = new;
        head.transmute().prev = new;
    }

    pub unsafe fn empty(head: *mut ListHead) -> bool {
        if head.transmute().next == head {
            true
        } else {
            false
        }
    }

    pub unsafe fn del(entry: *mut ListHead) {
        entry.transmute().prev.transmute().next = entry.transmute().next;
        entry.transmute().next.transmute().prev = entry.transmute().prev;

        ListHead::init(entry);
    }
}

pub struct ListIter {
    head: *mut ListHead,
    cur: *mut ListHead,
}

impl Iterator for ListIter {
    type Item = *mut ListHead;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur == self.head {
            return None;
        }
        let res = self.cur;
        self.cur = unsafe { self.cur.transmute().next };
        Some(res)
    }
}

#[macro_export]
macro_rules! container_of {
    ($ptr:expr, $container:ty, $($fields:expr)+ $(,)?) => {
        $ptr.byte_sub(std::mem::offset_of!($container, $($fields)+)) as *mut $container
    };
}

#[macro_export]
macro_rules! list_for_each_entry {
    ($container:ty, $head:expr, $($fields:expr)+, |$entry:ident| => $st:stmt) => {
        for _item in ListHead::iter($head) {
            let $entry = container_of!(_item, $container, $($fields)+);
            $st
        }
    };
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_linked_list() {
        let mut head: ListHead = ListHead::null();
        unsafe { ListHead::init(std::ptr::addr_of_mut!(head)) };

        struct Foo {
            val: usize,
            list: ListHead,
        }

        let mut foo = Foo {
            val: 1024,
            list: ListHead::null(),
        };
        unsafe { ListHead::init(std::ptr::addr_of_mut!(foo.list)) };

        let foo_ptr = unsafe { container_of!(std::ptr::addr_of_mut!(foo.list), Foo, list) };
        assert_eq!(foo_ptr, std::ptr::addr_of_mut!(foo));

        unsafe {
            ListHead::add(
                std::ptr::addr_of_mut!(head),
                std::ptr::addr_of_mut!(foo.list),
            )
        };
        let mut item_count = 0;

        unsafe {
            list_for_each_entry!(Foo, std::ptr::addr_of_mut!(head), list, |item| => {
                assert_eq!(item.transmute().val, 1024);
                item_count += 1;
            });
        }

        assert_eq!(item_count, 1);
    }
}
