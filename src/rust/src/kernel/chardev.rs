use core::{
    marker::PhantomData,
    ffi::{c_int, c_void, c_char},
};

use alloc::{sync::Arc, boxed::Box, ffi::CString};

use crate::prelude::*;
use crate::kernel::bindings;

#[doc(hidden)]
struct _InternalRegistration<T> {
    name: CString,
    dev: *mut bindings::nk_char_dev,
    data: *mut c_void,
    _p: PhantomData<Arc<T>>,
}

// SAFETY: `dev` and `data` are raw pointers with no thread affinity. The C
// code using them does not modify them or move
// their referrents. We only store them in an `_InternalRegistration`
// so that we can later reclaim the memory they point to. So it is
// safe to send an `_InternalRegistration` between threads. `Send`
// is important to implement here so that, if some type `T` contains
// an `chardev::Registration`, then `Mutex<NkIrqLock, T>` (from `lock_api`)
// implements `Sync` and `Send`.
//
// This is one of those `unsafe` lines that I cannot say with 100%
// confidence are actually safe. See also the comment for
// `irq::_InternalRegistration`.
unsafe impl<T> Send for _InternalRegistration<T> {}

impl<T> _InternalRegistration<T> {
    /// Registers `dev` with Nautilus' chararcter device subsytem.
    unsafe fn try_new(
        name: &str,
        interface: *mut bindings::nk_char_dev_int,
        data: Arc<T>,
    ) -> Result<Self> {
        let c_name = match CString::new(name) {
            Ok(s) => s,
            Err(_) => {
                error!("Cannot create C string from {name}.");
                return Err(-1)
            }
        };

        let ptr = Arc::into_raw(data) as *mut c_void;

        let dev = 
            // SAFETY: `name` will never be written to by the C code, and
            // will remain valid as long as the registration is alive.
            // Similarly, `ptr` also remains valid as long as the
            // registration is alive.
            unsafe {
                bindings::nk_char_dev_register(
                    c_name.as_ptr() as *mut c_char,
                    0,
                    interface,
                    ptr as *mut c_void,
                )
            };

        if dev.is_null() {
            error!("Unable to register device {}.", name);
            // SAFETY: `ptr` came from a previous call to `into_raw`.
            unsafe { let _ = Arc::from_raw(ptr); }
            Err(-1)
        } else {
            debug!("Successfully registered device {}.", name);
            Ok(Self {
                name: c_name,
                dev,
                data: ptr,
                _p: PhantomData,
            })
        }
    }
}

impl<T> Drop for _InternalRegistration<T> {
    fn drop(&mut self) {
        debug!("Dropping a registration for device {:?}.", self.name);

        let d = self.dev as *mut bindings::nk_char_dev;
        // SAFETY: Inside of `self.dev`, there is a pointer to the
        // chardev interface. This deallocation matches the call
        // to `Box::leak` in `try_new` in the success case.
        let _ = unsafe { Box::from_raw((*d).dev.interface) };

        // SAFETY: `self.dev` was successfully registered when the
        // registration was created, so it is non-null and safe to
        // deregister.
        unsafe { bindings::nk_char_dev_unregister(self.dev); }

        // SAFETY: This matches the call to `into_raw` from `try_new`
        // in the success case.
        unsafe { Arc::from_raw(self.data); }
    }
}

// Can't use `c_int` here. This shouldn't be a problem on normal systems.
#[repr(i32)]
pub enum StatusReturn {
    NotReady = 0,
    Readable = bindings::NK_CHARDEV_READABLE as _,
    Writable = bindings::NK_CHARDEV_WRITEABLE as _,
    ReadableAndWritable = (bindings::NK_CHARDEV_READABLE | bindings::NK_CHARDEV_WRITEABLE) as _,
    Error    = bindings::NK_CHARDEV_ERROR as _,
}

pub enum RwReturn<T = ()> {
    Ok(T),
    WouldBlock,
    Err
}

impl<T> core::convert::Into<c_int> for RwReturn<T> {
    fn into(self) -> c_int {
        match self {
            RwReturn::Ok(_) => 1,
            RwReturn::WouldBlock => 0,
            RwReturn::Err => -1
        }
    }
}

pub type Characteristics = bindings::nk_char_dev_characteristics;

pub trait Chardev {
    type State;

    fn status(state: &Self::State) -> StatusReturn;
    fn read(state: &Self::State) -> RwReturn<u8>;
    fn write(state: &Self::State, data: u8) -> RwReturn;
    fn get_characteristics(state: &Self::State) -> Result<Characteristics>;
}

pub struct Registration<C: Chardev>(_InternalRegistration<C::State>);

impl<C: Chardev> Registration<C> {
    unsafe extern "C" fn status(raw_state: *mut c_void) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the irq is unregistered.
        let state = unsafe { (raw_state as *const C::State).as_ref() }.unwrap();
        C::status(state) as _
    }

    unsafe extern "C" fn read(raw_state: *mut c_void, dest: *mut u8) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the irq is unregistered.
        let state = unsafe { (raw_state as *const C::State).as_ref() }.unwrap();
        
        let ret = C::read(state);
        match ret {
            RwReturn::Ok(v) => unsafe { *dest = v },
            _ => {}
        };

        ret.into()
    }

    unsafe extern "C" fn write(raw_state: *mut c_void, src: *mut u8) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the irq is unregistered.
        let state = unsafe { (raw_state as *const C::State).as_ref() }.unwrap();

        // SAFETY: the `src` presented to us by the chardev subsytem is not null.
        let data = unsafe { *src };
        
        let ret = C::write(state, data);
        ret.into()
    }


    unsafe extern "C" fn get_characteristics(
        raw_state: *mut c_void,
        c: *mut bindings::nk_char_dev_characteristics
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the irq is unregistered.
        let state = unsafe { (raw_state as *const C::State).as_ref() }.unwrap();

        let ret = C::get_characteristics(state);
        match ret {
            Ok(v) => {
                // SAFETY: `c` is a vaid pointer to `nk_char_dev_characteristics`.
                unsafe { *c = v };
                0
            },
            Err(_) => {
                -1
            }
        }

    }

    pub fn try_new(name: &str, data: Arc<C::State>) -> Result<Self> {
        let interface = Box::new(bindings::nk_char_dev_int {
            dev_int: bindings::nk_dev_int {
                open: None,
                close: None,
            },
            status: Some(Registration::<C>::status),
            read: Some(Registration::<C>::read),
            write: Some(Registration::<C>::write),
            get_characteristics: Some(Registration::<C>::get_characteristics)
        });

        let interface_ptr = Box::leak(interface);

        // SAFETY: `name`, `interface_ptr`, and `data` are all valid pointers.
        // The call to `Box::from_raw` matches the call to `Box::leak` in the
        // error case.
        Ok(Self(unsafe {
            _InternalRegistration::try_new(name, interface_ptr, data)
                .inspect_err(|_| { let _ = Box::from_raw(interface_ptr); })?
        }))
    }

    pub fn signal(&mut self) {
        if self.0.dev.is_null() {
            panic!("not registered");
        }

        let d = self.0.dev as *mut bindings::nk_dev;
        // SAFETY: `d` is a non-null pointer to `nk_dev`, guaranteed
        // by the existence of `self`.
        unsafe { bindings::nk_dev_signal(d); }
    }
}
