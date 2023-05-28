//! Thread Kernel Module
//!
//! This module provides a safe and idiomatic Rust interface for managing Nautilus threads.
//!
//! # Spawning a thread
//!
//! A new thread can be spawned using the [`thread::spawn`](spawn) function:
//!
//! ```
//! use crate::kernel::thread;
//!
//! thread::spawn(move || {
//!    // some work here
//! });
//! ```
//!
//! In this example, the spawned thread is “detached,” which means that there is no way
//! for the program to learn when the spawned thread completes or otherwise terminates.
//! 
//! To learn when a thread completes, it is necessary to capture the [`JoinHandle`] object
//! that is returned by the call to [`spawn`], which provides a [`join`][JoinHandle::join] method that allows the
//! caller to wait for the completion of the spawned thread:
//!
//! ```
//! use crate::kernel::thread;
//! 
//! let thread_join_handle = thread::spawn(move || {
//!     // some work here
//! });
//! // some work here
//! let output = thread_join_handle.join().expect("Thread failed to join!");
//! ```
//!
//! # Configuring threads
//!
//! TODO
//!
//! # Thread-local storage
//!
//! TODO

use alloc::{boxed::Box, ffi::CString};
use core::{ffi::c_void, cell::UnsafeCell, mem::MaybeUninit};

use crate::kernel::{bindings, error::{Result, ResultExt}, print::make_logging_macros};

make_logging_macros!("thread");

unsafe extern "C" fn call_closure<F, T>(raw_input: *mut c_void, _: *mut *mut c_void)
where
    F: FnMut() -> T,
{
    // SAFETY: The C caller makes sure that `raw_input` is the pointer we passed
    // when we called `nk_thread_start`. The referrent of that pointer was a
    // `(F, MaybeUninit<T>)` tuple, so it is safe to dereference it here as such.
    let (callback, output) = unsafe { &mut *(raw_input as *mut (F, MaybeUninit<T>)) };
    output.write(callback());
}

pub type ThreadId = bindings::nk_thread_id_t;

/// An owned permission to join on a thread (block on its termination).
/// 
/// when a `JoinHandle` is dropped there is no longer any handle to the
/// thread and no way to `join` on it, and the memory associated with the
/// thread function and its output is forgotten and cannot be deallocated.
/// 
/// This struct is created by the [`thread::spawn`][spawn] function and the
/// (TODO) `thread::Builder::spawn` method.
#[derive(Debug)]
pub struct JoinHandle<F, T> {
    id: ThreadId,
    data: Box<Option<UnsafeCell<(F, MaybeUninit<T>)>>> // awful awful awful
}

/// The possible stack sizes for a thread.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum StackSize {
    Default = bindings::TSTACK_DEFAULT,
    _4KB = bindings::TSTACK_4KB,
    _1MB = bindings::TSTACK_1MB,
    _2MB = bindings::TSTACK_2MB,
}

impl<F, T> JoinHandle<F, T> {
    /// Waits for the associated thread to finish.
    ///
    /// This function will return immediately if the associated thread has already finished.
    pub fn join(mut self) -> Result<T> {
        // SAFETY: `nk_join` is a C function that expects a valid thread id that was returned from a successful
        // thread creation. It also expects a valid pointer for storing thread output.
        let ret = unsafe { bindings::nk_join(self.id, core::ptr::null_mut() as _) };

        Result::from_error_code(ret).map(|_| { unsafe { self.data.take().unwrap().into_inner().1.assume_init() } })
    }

    /// Names the thread.
    ///
    /// # Arguments
    ///
    /// `name` - The name for the thread.
    ///
    /// # Returns
    ///
    /// Returns an empty `Result` on success, or a `ThreadError` on failure.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it calls the C function `nk_thread_name`.
    /// The safety of this function depends on the correct implementation of the C library.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::kernel::thread::Thread;
    ///
    /// let thread = unsafe { Thread::create(None, None, None, false, ThreadStackSize::Default, -1).unwrap() };
    /// let result = thread.name("MyThread");
    /// assert!(result.is_ok());
    /// ```
    pub fn set_name(&self, name: &str) -> Result {
        let cstr = CString::new(name).unwrap();

        // SAFETY: `nk_thread_name` is a C function that expects a valid
        // thread id and a valid null-terminated string pointer for the thread name.
        // If successful, it sets the thread name and returns 0.
        let ret = unsafe { bindings::nk_thread_name(self.id, cstr.as_ptr() as *mut i8) };
        Result::from_error_code(ret)
    }

}

impl<F, T> Drop for JoinHandle<F, T> {
    fn drop(&mut self) {
        // TODO: should we manually destroy the thread here,
        // or let them be reaped by the scheduler?

        if let Some(ref mut thread_data) = *self.data {
            // If the `JoinHandle` was dropped with the inner data still
            // alive, then the thread was never joined-on, and may still
            // be running. Running a destructor on memory it's using
            // would be UB, so we have to leak it here.
            core::mem::forget(thread_data);
        } 
    }
}

/// Spawns a new thread, returning a [`JoinHandle`] for it.
/// 
/// The join handle provides a [`join`][JoinHandle::join] method that can be used to join the
/// spawned thread. 
/// 
/// If the join handle is dropped, the spawned thread may no longer be joined. (It is the
/// responsibility of the program to either eventually join threads it creates or detach them;
/// otherwise, a resource leak will result.)
/// 
/// This call will create a thread using default parameters of (TODO) `Builder`, if you want to specify the
/// stack size or the name of the thread, use this API instead.
/// 
/// As you can see in the signature of `spawn` there are two constraints on both the closure given to
/// spawn and its return value, let’s explain them:
/// 
/// - The `'static` constraint means that the closure and its return value must have a lifetime of
///   the whole program execution. The reason for this is that threads can outlive the lifetime
///   they have been created in.
///   
///   Indeed if the thread, and by extension its return value, can outlive their caller, we need
///   to make sure that they will be valid afterwards, and since we can’t know when it will
///   return we need to have them valid as long as possible, that is until the end of the
///   program, hence the `'static` lifetime.
/// 
/// - The `Send` constraint is because the closure will need to be
///   passed by value from the thread where it is spawned to the new thread. Its return value
///   will need to be passed from the new thread to the thread where it is joined. As a reminder,
///   the Send marker trait expresses that it is safe to be passed from thread to thread.
///   `Sync` expresses that it is safe to have a reference be passed from
///   thread to thread.
pub fn spawn<F, T>(
    f: F,
    is_detached: bool,
    stack_size: StackSize,
    bound_cpu: i32,
) -> JoinHandle<F, T>
where
    F: FnMut() -> T + Send + 'static,
    T: Send + 'static
{
    let mut id = core::ptr::null_mut();
    let data = Box::new(Some(UnsafeCell::new((f, MaybeUninit::uninit()))));

    // SAFETY: `nk_thread_start` is a C function that expects valid
    // function pointers, input pointer, output pointer and stack
    // size for successful thread creation and running. If successful,
    // it returns a valid pointer in 'tid' that can be used in other
    // thread operations.
    let ret = unsafe {
        bindings::nk_thread_start(
            Some(call_closure::<F, T>),
            (*data).as_ref().unwrap().get() as *mut _,
            core::ptr::null_mut(),
            is_detached as u8,
            stack_size as u64,
            &mut id,
            bound_cpu,
        )
    };

    if ret != 0 {
        panic!("Thread failed to spawn! Error code {ret}");
    }

    JoinHandle { id, data }
}


/// Causes the thread to yield execution to another thread that is ready to run.
pub fn thread_yield() {
    // SAFETY: `nk_yield` is a C function that causes the calling thread to yield execution to another thread that
    // is ready to run and does not return a value.
    unsafe { bindings::nk_yield(); }
}


/// Gets the thread id of the current thread.
/// Cannot be used with the LEGION runtime.
pub fn get_tid() -> ThreadId {
    // `bindings::nk_get_tid` will not exist if the LEGION runtime has been built
    // into the kernel. We don't want everything to fail to compile if LEGION is built, 
    // so we introduce a runtime panic if this function if called with `__LEGION__` defined.
    //
    // Ideally, we would compile our own `Thread::get_tid` only if `__LEGION__` is
    // not defined (like the C code does), but `#[cfg(not(accessible(...))]` is not
    // a thing yet.
    #[cfg_accessible(crate::kernel::bindings::__LEGION__)]
    panic!("Thread::get_tid cannot be used with the LEGION runtime.");

    // SAFETY: `nk_get_tid` is a C function that gets the current thread id.
    unsafe { bindings::nk_get_tid() }
}

/// Gets the thread id of the parent thread.
///
/// # Returns
///
/// The thread id of the parent thread.
///
/// # Safety
///
/// This function is unsafe because it calls the C function `nk_get_parent_tid`.
/// The safety of this function depends on the correct implementation of the C library.
pub fn get_parent_tid() -> ThreadId {
    // SAFETY: `nk_get_parent_tid` is a C function that gets the parent thread id.
    unsafe { bindings::nk_get_parent_tid() }
}

/// Exits the thread.
///
/// Called explictly or implicitly when a thread exits.
///
/// # Arguments
///
/// `retval` - A pointer to the return value.
///
/// # Safety
///
/// This function is unsafe because it calls the C function `nk_thread_exit`.
/// The safety of this function depends on the correct implementation of the C library.
pub fn exit(retval: *mut c_void) {
    // SAFETY: `nk_thread_exit` is a C function that expects a valid pointer for the return value.
    // It causes the calling thread to terminate and does not return a value.
    unsafe { bindings::nk_thread_exit(retval) }
}

/// Fork the current thread.
///
/// # Returns
///
/// Returns the thread id of the child to the parent thread, or zero to the child thread.
/// On error, returns `NK_BAD_THREAD_ID`.
///
/// # Safety
///
/// This function is unsafe because it calls the C function `nk_thread_fork`.
/// The safety of this function depends on the correct implementation of the C library.
pub fn fork() -> ThreadId {
    // SAFETY: `nk_thread_fork` is a C function that forks the current thread.
    // If successful, it returns the thread id of the child to the parent thread, or zero to the child thread.
    unsafe { bindings::nk_thread_fork() }
}


