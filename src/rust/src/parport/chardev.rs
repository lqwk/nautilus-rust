const CHARDEV_RW: c_int =
    (nk_bindings::NK_CHARDEV_READABLE | nk_bindings::NK_CHARDEV_WRITEABLE) as c_int;

use core::{
    ffi::{c_int, c_void},
    fmt::Error,
    intrinsics::write_bytes,
    ptr::NonNull,
};

use alloc::string::String;

use crate::{nk_bindings, utils::to_c_string};

use super::Parport;

pub struct NkCharDev<'a> {
    dev: &'a mut nk_bindings::nk_char_dev,
}

impl<'a> NkCharDev<'a> {
    pub fn get_name(&self) -> String {
        let name = &self.dev.dev.name[..];
        // have [i8], want [u8] - safe in this context
        let s = unsafe { &*(name as *const [i8] as *const [u8]) };
        String::from_utf8_lossy(s).into_owned()
    }

    pub fn new() -> Result<Self, Error> {
        // TODO: register
    }

    // does this function need to be `unsafe`?
    unsafe fn nk_char_dev_register(
        name: &str,
        parport: &'a mut Parport,
    ) -> Result<NonNull<nk_bindings::nk_char_dev>, Error> {
        let name_bytes = to_c_string(name);
        let parport_ptr = parport as *const Parport;
        let cd = &CHARDEV_INTERFACE as *const nk_bindings::nk_char_dev_int;
        let r;
        unsafe {
            r = nk_bindings::nk_char_dev_register(
                name_bytes,
                0,
                // not actually mutable, but C code had no `const` qualifier
                cd as *mut nk_bindings::nk_char_dev_int,
                // not actually mutable, but C code had no `const` qualifier
                parport_ptr as *mut Parport as *mut c_void,
            );
        }
        NonNull::new(r).ok_or(Error)
    }
}

impl<'a> Drop for NkCharDev<'a> {
    fn drop(&mut self) {
        let ptr = self.dev as *mut nk_bindings::nk_char_dev;
        unsafe {
            nk_bindings::nk_char_dev_unregister(ptr);
        }
    }
}

pub unsafe extern "C" fn status(state: *mut c_void) -> c_int {
    let p = state as *const Parport;
    let p = unsafe { p.as_ref() }.unwrap();

    unimplemented!()
}

pub unsafe extern "C" fn read(state: *mut c_void, dest: *mut u8) -> c_int {
    unimplemented!()
}

pub unsafe extern "C" fn write(state: *mut c_void, src: *mut u8) -> c_int {
    unimplemented!()
}

pub unsafe extern "C" fn get_characteristics(
    _state: *mut c_void,
    c: *mut nk_bindings::nk_char_dev_characteristics,
) -> c_int {
    unsafe {
        // memset the (single) struct to bytes of 0
        write_bytes(c, 0, 1);
    }
    0
}

const CHARDEV_INTERFACE: nk_bindings::nk_char_dev_int = nk_bindings::nk_char_dev_int {
    get_characteristics: Some(get_characteristics),
    read: Some(read),
    write: Some(write),
    status: Some(status),
    dev_int: nk_bindings::nk_dev_int {
        open: None,
        close: None,
    },
};
