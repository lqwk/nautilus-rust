const CHARDEV_RW: c_int =
    (nk_bindings::NK_CHARDEV_READABLE | nk_bindings::NK_CHARDEV_WRITEABLE) as c_int;

use core::{
    ffi::{c_int, c_void},
    fmt::Error,
    intrinsics::write_bytes,
    ptr::null_mut,
};

use alloc::{borrow::ToOwned, string::String, sync::Arc};

use crate::utils::print_to_vc;
use crate::{nk_bindings, utils::to_c_string};

use super::{lock::IRQLock, Parport};

pub struct NkCharDev {
    dev: *mut nk_bindings::nk_char_dev,
    name: String,
}

impl NkCharDev {
    pub fn get_name(&self) -> String {
        self.name.to_owned()
    }

    pub fn new(name: &str) -> Self {
        Self {
            dev: null_mut(),
            name: name.to_owned(),
        }
    }

    pub fn signal(&mut self) {
        if self.dev.is_null() {
            panic!("not registered");
        }

        let d = self.dev as *mut nk_bindings::nk_dev;
        unsafe {
            nk_bindings::nk_dev_signal(d);
        }
    }

    pub fn register(&mut self, parport: Arc<IRQLock<Parport>>) -> Result<(), Error> {
        print_to_vc("register device\n");

        if !self.dev.is_null() {
            panic!("attempted to register NkCharDev twice");
        }

        // TODO: fix leak of this C string on unregistration
        let name_bytes = to_c_string(&self.name);
        let parport_ptr = Arc::into_raw(parport);
        let cd = &CHARDEV_INTERFACE as *const nk_bindings::nk_char_dev_int;
        let r;
        unsafe {
            r = nk_bindings::nk_char_dev_register(
                name_bytes,
                0,
                // not actually mutable, but C code had no `const` qualifier
                cd as *mut nk_bindings::nk_char_dev_int,
                // not actually mutable, but C code had no `const` qualifier
                parport_ptr as *mut c_void,
            );
        }

        self.dev = r;
        (!r.is_null()).then(|| ()).ok_or(Error)
    }
}

impl Drop for NkCharDev {
    fn drop(&mut self) {
        if let Some(ptr) = unsafe { self.dev.as_mut() } {
            unsafe {
                nk_bindings::nk_char_dev_unregister(ptr);
            }
        }
    }
}

unsafe fn deref_locked_state(state: *mut c_void) -> Arc<IRQLock<Parport>> {
    // caller must guarantee `state`, and the object it points to, was not mutated
    unsafe { Arc::from_raw(state as *const IRQLock<Parport>) }
}

pub unsafe extern "C" fn status(state: *mut c_void) -> c_int {
    let p = unsafe { deref_locked_state(state) };
    if p.lock().is_ready() {
        CHARDEV_RW
    } else {
        0
    }
}

pub unsafe extern "C" fn read(state: *mut c_void, dest: *mut u8) -> c_int {
    print_to_vc("read!\n");

    let s = unsafe { deref_locked_state(state) };
    let mut p = s.lock();
    match p.read() {
        Ok(v) => {
            unsafe {
                // caller guarantees `dest` points to the correct byte to write into
                *dest = v;
            }
            1
        }
        Err(_) => 0,
    }
}

pub unsafe extern "C" fn write(state: *mut c_void, src: *mut u8) -> c_int {
    print_to_vc("write!\n");

    let s = unsafe { deref_locked_state(state) };
    let mut p = s.lock();
    // caller guarantees `src` points to the correct byte to write
    let byte = unsafe { *src };
    match p.write(byte) {
        Ok(_) => 1,  // success
        Err(_) => 0, // failure
    }
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
