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

    pub fn register(&mut self, deivce: Arc<IRQLock<T>>) -> Result {
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
// SAFETY: This function is marked unsafe due to two primary safety conditions that must be upheld by the caller:
//
// 1. The caller must guarantee that the `state` pointer is valid, i.e., it is not null, it points to an initialized 
//    `IRQLock<T>`, and the object it points to has not been mutated.
//
// 2. The caller must also ensure that the reference count of the containing `Arc` is not dropped to zero while the 
//    returned reference exists. This is to ensure that the object the returned reference points to is not deallocated 
//    prematurely.
//
// If these conditions are met, it is safe to call this function. If the `state` pointer is not valid or the 
// reference count of the `Arc` is dropped to zero while the reference is still in use, this function can cause 
// undefined behavior.
unsafe fn deref_locked_state<'a, T: CharDevOps>(state: *mut c_void) -> &'a IRQLock<T> {
    // caller must guarantee `state`, and the object it points to, was not mutated
    //
    // caller must not drop the strong reference count of the containing `Arc` to 0 while
    // the returned reference exists
    let l = state as *const IRQLock<T>;
    unsafe { l.as_ref() }.unwrap()
}

// SAFETY: This pointer is guaranteed to be valid. The caller ensures that the `state`
// pointer provided to the function is non-null and points to valid memory. Furthermore,
// the `deref_locked_state` function guarantees that the object to which `state` points
// has not been mutated. This, combined with the fact that the reference count of the 
// containing `Arc` is not dropped to 0 while the reference returned by `deref_locked_state`
// exists, ensures safety. 
pub unsafe extern "C" fn status<T: CharDevOps>(state: *mut c_void) -> c_int {
    let p = unsafe { deref_locked_state::<T>(state) };
    if p.lock().is_ready() {
        CHARDEV_RW
    } else {
        0
    }
}

// SAFETY: There are two primary safety concerns in this function.
// 
// 1. The `state` pointer provided by the caller is assumed to be a valid 
//    and properly initialized pointer to a `c_void`. The `deref_locked_state` function,
//    which is used to dereference this pointer, assumes that the pointer is valid, 
//    and that the object it points to has not been mutated. The caller must ensure 
//    that these conditions are met to guarantee safety.
// 
// 2. The `dest` pointer must point to a valid memory location where the read byte 
//    can be safely written. The caller guarantees this, and `*dest = v` only 
//    dereferences `dest` after this guarantee. The function does not check the 
//    validity of `dest`, so it's crucial that the caller ensures `dest` is a valid 
//    pointer to a `u8`.
//
// The caller is responsible for ensuring that the `state` and `dest` pointers 
// provided meet these conditions, and that the `Arc` containing `state` is not 
// dropped while the returned reference exists. If these conditions are met, 
// the function is safe to call.

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
// SAFETY: This function is marked as unsafe due to two key safety conditions that must be upheld by the caller:
//
// 1. The `state` pointer provided to this function should be a valid pointer to a `c_void`. The `deref_locked_state` 
//    function, which is used to dereference this pointer, assumes that it is valid and that the object it points to has 
//    not been mutated. The caller must ensure these conditions are met to guarantee safety.
//
// 2. The `src` pointer should point to a valid memory location containing the byte to be written. The caller 
//    guarantees this and the function does not validate it. The function dereferences `src` after this guarantee 
//    (`*src`), so it's crucial that the caller ensures `src` is a valid pointer to a `u8`.
//
// If these conditions are met, the function is safe to call. If these assumptions about the `state` and `src` 
// pointers are not accurate, this function can lead to undefined behavior.
pub unsafe extern "C" fn write<T: CharDevOps>(state: *mut c_void, src: *mut u8) -> c_int {
    debug!("write!");

    let s = unsafe { deref_locked_state::<T>(state) };
    let mut p = s.lock();
    // SAFETY: Caller guarantees `src` points to the correct byte to write.
    let byte = unsafe { *src };
    match p.write(byte) {
        Ok(_) => 1,  // success
        Err(_) => 0, // failure
    }
}
// SAFETY: This function is marked as unsafe because it has two primary safety conditions that the caller must uphold:
//
// 1. The `_state` pointer provided by the caller should be a valid pointer to a `c_void`. Even though this function 
//    does not dereference `_state`, the caller must ensure that this pointer is valid and that the object it points to 
//    has not been mutated to maintain the safety guarantees of the calling context.
//
// 2. The `c` pointer should point to a valid memory location where the `nk_char_dev_characteristics` can be safely 
//    written. The function uses `write_bytes(c, 0, 1)` to write to this memory, and does not validate the pointer `c`. 
//    This means it's crucial for the caller to ensure that `c` is a valid pointer to an `nk_char_dev_characteristics`.
//
// If these conditions are met, the function is safe to call. If these assumptions about the `_state` and `c` 
// pointers are not accurate, this function can lead to undefined behavior.
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