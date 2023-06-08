use core::{
    marker::PhantomData,
    ffi::{c_int, c_void, c_char},
};

use alloc::{sync::Arc, boxed::Box, ffi::CString};

use crate::kernel::{
    error::Result,
    print::make_logging_macros,
    bindings
};

make_logging_macros!("chardev");

/// Manages resources associated with registering a character
/// device.
/// 
/// # Invariants
///
/// `dev` and `data` are valid, non-null pointers.
#[derive(Debug)]
struct InternalRegistration<T> {
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
unsafe impl<T> Send for InternalRegistration<T> {}

impl<T> InternalRegistration<T> {
    /// Registers a character device with Nautilus' character device subsytem.
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
            unsafe { let _ = Arc::from_raw(ptr as *mut T); }
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

impl<T> Drop for InternalRegistration<T> {
    fn drop(&mut self) {
        debug!("Dropping a registration for device {:?}.", self.name);

        // SAFETY: `self.dev` was successfully registered when the
        // registration was created, so it is non-null and safe to
        // deregister.
        unsafe { bindings::nk_char_dev_unregister(self.dev); }

        // SAFETY: This matches the call to `into_raw` from `try_new`
        // in the success case.
        unsafe { Arc::from_raw(self.data as *mut T); }
    }
}

/// The return type of the `status` function for character devices.
#[repr(i32)] // Can't use `c_int` here. This shouldn't be a problem on normal systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The device is not ready for read/write operations right now.
    Busy = 0,
    /// The device is available for reading, but not writing.
    Readable = bindings::NK_CHARDEV_READABLE as _,
    /// The device is available for writing, but not reading.
    Writable = bindings::NK_CHARDEV_WRITEABLE as _,
    /// The device is available for both reading and writing.
    ReadableAndWritable = (bindings::NK_CHARDEV_READABLE | bindings::NK_CHARDEV_WRITEABLE) as _,
    /// The device is in an erroneous state.
    Error = bindings::NK_CHARDEV_ERROR as _,
}

/// The return type of the `read` and `write` functions for character devices.
/// Nautilus requires that these functions do not block, and so it defines
/// three possible return statuses: success, failure, and not ready.
#[derive(Debug)]
pub enum RwResult<T = ()> {
    /// The read/write succeeded. `T` should be the value read if it
    /// was a `read` operation, or `()` if it was a `write`.
    Ok(T),
    /// The read/write did not occur because it would have blocked.
    WouldBlock,
    /// There was an error while reading/writing.
    Err
}

impl<T> core::convert::From<RwResult<T>> for c_int {
    fn from(val: RwResult<T>) -> Self {
        match val {
            // It may be surprising that `0` indicates would-have-blocked
            // and not success, but that's just the way it is in Nautilus.
            RwResult::Ok(_) => 1,
            RwResult::WouldBlock => 0,
            RwResult::Err => -1
        }
    }
}

/// Characteristics of the character device. Currently, this
/// is a zero-sized type in Nautilus.
pub type Characteristics = bindings::nk_char_dev_characteristics;

/// A Nautilus character device.
pub trait CharDev {
    /// The state associated with the character device.
    type State: Send + Sync;

    /// Checks the devices status. Can be readable, writable,
    /// both, neither, or in a erroneous state.
    fn status(state: &Self::State) -> Status;

    /// Reads one byte from the character device.
    fn read(state: &Self::State) -> RwResult<u8>;

    /// Write one byte to the character device.
    fn write(state: &Self::State, data: u8) -> RwResult;

    /// Gets the characteristics of the character device.
    /// Currently, character devices have no characteristics
    /// in Nautilus, so `Characteristics` is a zero-sized type.
    fn get_characteristics(state: &Self::State) -> Result<Characteristics>;
}

/// The registration of a character device.
#[derive(Debug)]
pub struct Registration<C: CharDev>(InternalRegistration<C::State>);

impl<C: CharDev> Registration<C> {
    unsafe extern "C" fn status(raw_state: *mut c_void) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const C::State) };
        C::status(state) as _
    }

    unsafe extern "C" fn read(raw_state: *mut c_void, dest: *mut u8) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const C::State) };
        
        let ret = C::read(state);
        if let RwResult::Ok(v) = ret {
            // SAFETY: Caller ensures `dest` is a valid pointer.
            unsafe { *dest = v }; 
        }

        ret.into()
    }

    unsafe extern "C" fn write(raw_state: *mut c_void, src: *mut u8) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const C::State) };

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
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const C::State) };

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

    /// Registers a character device with Nautilus' character device subsytem.
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

        // Don't clean up this memory while the C code uses it.
        // We could have used `Arc` (and `into_raw`) here instead of a
        // `Box`, but the C code is the sole owner of the memory during
        // its lifetime.
        //
        // In theory, we also could have put `interface` in static
        // memory (since it is built of values known at compile-time),
        // but Rust does not have generic statics at the moment, and
        // we can't use `C` from the outer `impl` in that declaration.
        let interface_ptr = Box::into_raw(interface);

        // SAFETY: `name`, `interface_ptr`, and `data` are all valid pointers.
        // The call to `Box::from_raw` matches the call to `Box::into_raw` in the
        // error case.
        Ok(Self(unsafe {
            InternalRegistration::try_new(name, interface_ptr, data)
                .inspect_err(|_| { let _ = Box::from_raw(interface_ptr); })?
        }))
    }

    /// Wakes up threads waiting on the character device.
    pub fn signal(&mut self) {
        if self.0.dev.is_null() {
            panic!("not registered");
        }

        let d = self.0.dev as *mut bindings::nk_dev;
        // SAFETY: `d` is a non-null pointer to `nk_dev`, guaranteed
        // by the existence of `self`.
        unsafe { bindings::nk_dev_signal(d); }
    }

    /// Gets the name of the character device.
    pub fn name(&self) -> &str {
        self.0.name.to_str().expect("Name cannot contain internal null bytes.")
    }
}

impl<C: CharDev> Drop for Registration<C> {
    fn drop(&mut self) {
        let d = self.0.dev as *mut bindings::nk_char_dev;

        // SAFETY: Inside of `self.0.dev`, there is a pointer to the
        // chardev interface. This deallocation matches the call
        // to `Box::into_raw` in `Registration::try_new` in the success case.
        //
        // Note that we could have done this deallocation in `drop`
        // for `_InternalRegistration`, but this would technically
        // be dangerous if someone created an `_InternalRegistration`
        // without `Registration::try_new` (which no-one should ever do).
        // Anyway, it fits best here.
        let _ = unsafe { Box::from_raw((*d).dev.interface) };
    }
}
