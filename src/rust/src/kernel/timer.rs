/// Create a new timer
///
/// # Arguments
///
/// `name` - A string for the name of the timer
///
/// # Returns
///
/// A new `Timer` struct.
///
/// # Safety
///
/// This function is unsafe because it calls the C function `nk_timer_create`.
/// The safety of this function depends on the correct implementation of the C library.
///
/// # Examples
///
/// ```
/// use crate::timer::Timer;
///
/// let timer = Timer::new("my_timer");
/// ```
use core::ffi::c_void;
use alloc::ffi::CString;

use super::bindings;

#[derive(Debug)]
pub struct Timer {
    timer: *mut bindings::nk_timer_t,
}

impl Timer {
    pub fn new(name: &str) -> Option<Self> {
        let c_name = CString::new(name).unwrap();
        // SAFETY: `nk_timer_create` returns a valid pointer on success, or null on failure.
        // The returned pointer is stored in the Timer struct and is later used in other
        // functions that expect a valid pointer.
        let timer = unsafe { bindings::nk_timer_create(c_name.as_ptr() as *mut _) };
        if timer.is_null() {
            None
        } else {
            Some(Self { timer })
        }
    }

    /// # Safety
    ///
    /// The data behind `priv_data` will not be overwritten as a result of
    /// this call, although the C function being called will create an alias to it.
    /// This timer API as a whole needs to be improved--if you are coming across this
    /// because you need this functionality, you may want to make something more idiomatic.
    pub unsafe fn set(
        &self,
        ns: u64,
        flags: u64,
        callback: Option<unsafe extern "C" fn(*mut c_void)>, // Add unsafe keyword here
        priv_data: *mut c_void,
        cpu: i32,
    ) -> i32 {


        // SAFETY: `nk_timer_set` expects a valid timer pointer, which is guaranteed by the
        // `new` function. The other arguments are passed directly from the caller.
        unsafe {
            bindings::nk_timer_set(
                self.timer,
                ns,
                flags,
                callback,
                priv_data,
                cpu as u32, // Convert to u32
            )
        }
    }    

    pub fn reset(&self, ns: u64) -> i32 {
        // SAFETY: `nk_timer_reset` expects a valid timer pointer, which is guaranteed by the
        // `new` function.
        unsafe { bindings::nk_timer_reset(self.timer, ns) }
    }

    pub fn start(&self) -> i32 {
        // SAFETY: `nk_timer_start` expects a valid timer pointer, which is guaranteed by the
        // `new` function.
        unsafe { bindings::nk_timer_start(self.timer) }
    }

    pub fn cancel(&self) -> i32 {
        // SAFETY: `nk_timer_cancel` expects a valid timer pointer, which is guaranteed by the
        // `new` function.
        unsafe { bindings::nk_timer_cancel(self.timer) }
    }

    pub fn wait(&self) -> i32 {
        // SAFETY: `nk_timer_wait` expects a valid timer pointer, which is guaranteed by the
        // `new` function.
        unsafe { bindings::nk_timer_wait(self.timer) }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        // SAFETY: `nk_timer_destroy` expects a valid timer pointer, which is guaranteed by the
        // `new` function. This function is called when the Timer struct is dropped, which
        // ensures that the timer is properly destroyed.
        unsafe { bindings::nk_timer_destroy(self.timer) };
    }
}

pub fn get_thread_default() -> Option<Timer> {
    // SAFETY: `nk_timer_get_thread_default` returns a valid pointer on success, or null on failure.
    // The returned pointer is stored in the Timer struct and is later used in other
    // functions that expect a valid pointer.
    let timer = unsafe { bindings::nk_timer_get_thread_default() };
    if timer.is_null() {
        None
    } else {
        Some(Timer { timer })
    }
}

pub fn get_realtime() -> u64 {
    // SAFETY: `nk_sched_get_realtime` is a simple function that takes no arguments. There are no
    // safety concerns as long as the underlying C library is correctly implemented.
    unsafe { bindings::nk_sched_get_realtime() }
}
