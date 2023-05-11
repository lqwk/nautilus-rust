const CHARDEV_RW: c_int = (bindings::NK_CHARDEV_READABLE | bindings::NK_CHARDEV_WRITEABLE) as c_int;

use core::{
    ffi::{c_int, c_void},
    intrinsics::write_bytes,
    ptr::null_mut,
};
use core::marker::PhantomData;

use alloc::{borrow::ToOwned, string::String, sync::Arc};

use crate::kernel::bindings;
use crate::kernel::utils::to_c_string;

use crate::prelude::*;

use super::{lock::IRQLock, Parport};

pub trait CharDevOps {
    fn is_ready(&mut self) -> bool;
    fn read(&mut self) -> Result<u8>;
    fn write(&mut self, byte: u8) -> Result<()>;
}



pub struct NkCharDev<T> {
    dev: *mut bindings::nk_char_dev,
    name: String,
    _p: PhantomData<Arc<T>>,
}

impl<T: CharDevOps> NkCharDev<T> {
    pub fn get_name(&self) -> String {
        self.name.to_owned()
    }

    pub fn new(name: &str) -> Self {
        Self {
            dev: null_mut(),
            name: name.to_owned(),
            _p: PhantomData,
        }
    }

    pub fn signal(&mut self) {
        if self.dev.is_null() {
            panic!("not registered");
        }

        let d = self.dev as *mut bindings::nk_dev;
        unsafe {
            bindings::nk_dev_signal(d);
        }
    }

    pub fn register(&mut self, deivce: Arc<IRQLock<T>>) -> Result<()> {
        debug!("register device");

        if !self.dev.is_null() {
            panic!("attempted to register NkCharDev twice");
        }

        // TODO: fix leak of this C string on unregistration
        let name_bytes = to_c_string(&self.name);
        let device_ptr = Arc::into_raw(deivce);
        let cd = &chardev_interface::<T>() as *const bindings::nk_char_dev_int;
        let r;
        unsafe {
            r = bindings::nk_char_dev_register(
                name_bytes,
                0,
                // not actually mutable, but C code had no `const` qualifier
                cd as *mut bindings::nk_char_dev_int,
                // not actually mutable, but C code had no `const` qualifier
                device_ptr as *mut c_void,
            );
        }

        self.dev = r;

        if r.is_null() {
            Err(-1)
        } else {
            Ok(())
        }
    }
}

impl<T> Drop for NkCharDev<T> {
    fn drop(&mut self) {
        if let Some(ptr) = unsafe { self.dev.as_mut() } {
            unsafe {
                // taking back `Arc` is safe from any non-null `chardev` we registered
                let _ = Arc::from_raw(ptr.dev.state as *const IRQLock<T>);
                bindings::nk_char_dev_unregister(ptr);
            }
        }
    }
}

unsafe fn deref_locked_state<'a, T: CharDevOps>(state: *mut c_void) -> &'a IRQLock<T> {
    // caller must guarantee `state`, and the object it points to, was not mutated
    //
    // caller must not drop the strong reference count of the containing `Arc` to 0 while
    // the returned reference exists
    let l = state as *const IRQLock<T>;
    unsafe { l.as_ref() }.unwrap()
}

pub unsafe extern "C" fn status<T: CharDevOps>(state: *mut c_void) -> c_int {
    let p = unsafe { deref_locked_state::<T>(state) };
    if p.lock().is_ready() {
        CHARDEV_RW
    } else {
        0
    }
}

pub unsafe extern "C" fn read<T: CharDevOps>(state: *mut c_void, dest: *mut u8) -> c_int {
    debug!("read!");

    let s = unsafe { deref_locked_state::<T>(state) };
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

pub unsafe extern "C" fn write<T: CharDevOps>(state: *mut c_void, src: *mut u8) -> c_int {
    debug!("write!");

    let s = unsafe { deref_locked_state::<T>(state) };
    let mut p = s.lock();
    // caller guarantees `src` points to the correct byte to write
    let byte = unsafe { *src };
    match p.write(byte) {
        Ok(_) => 1,  // success
        Err(_) => 0, // failure
    }
}

pub unsafe extern "C" fn get_characteristics<T: CharDevOps>(
    _state: *mut c_void,
    c: *mut bindings::nk_char_dev_characteristics,
) -> c_int {
    unsafe {
        // memset the (single) struct to bytes of 0
        write_bytes(c, 0, 1);
    }
    0
}



fn chardev_interface<T: CharDevOps>() -> bindings::nk_char_dev_int {
    bindings::nk_char_dev_int {
        get_characteristics: Some(get_characteristics::<T>),
        read: Some(read::<T>),
        write: Some(write::<T>),
        status: Some(status::<T>),
        dev_int: bindings::nk_dev_int {
            open: None,
            close: None,
        },
    }
}