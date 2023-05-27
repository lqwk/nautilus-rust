//! Thread Kernel Module
//!
//! This module provides a safe and idiomatic Rust interface for managing Nautilus threads.
//!
//! The main struct provided by this module is `Thread`. This struct acts as a handle for a
//! Nautilus thread, and provides methods for creating and controlling threads.
//!
//! # Example
//!
//! ```
//! use crate::kernel::thread::{Thread, ThreadStackSize};
//!
//! unsafe {
//!     let thread = Thread::new(None, None, None, false, ThreadStackSize::Default);
//! }
//! ```
//!
//! # Safety
//!
//! Many operations on threads are unsafe, because they involve dereferencing raw pointers,
//! calling FFI functions, or manipulating the thread of execution. The functions in this module
//! carefully uphold the necessary invariants to ensure memory safety, but it's important to
//! understand the underlying model when using this API.
use alloc::{boxed::Box, ffi::CString};
use core::ffi::c_void;

use crate::kernel::{bindings, error::{Result, ResultExt}, print::make_logging_macros};

make_logging_macros!("gpudev");

unsafe extern "C" fn call_closure<F, T>(raw_input: *mut c_void, raw_output: *mut *mut c_void)
where
    F: FnMut() -> T,
{
    debug!("{:?}", raw_input);
    let callback = unsafe { &mut *(raw_input as *mut F) };
    debug!("here");
    let out = callback();
    debug!("there");
    unsafe { **(raw_output as *mut *mut T) = out; }
}

pub type ThreadId = *mut c_void;
pub type ThreadFun = unsafe extern "C" fn(*mut c_void, *mut *mut c_void);

/// A Rust-friendly wrapper for a Nautilus thread.
///
/// This struct contains the thread ID as well as the function,
/// input, and output for the thread. It also includes information
/// about the stack size and whether or not the thread is detached.
///
/// # Safety
///
/// This struct is unsafe to use directly, as it requires proper
/// initialization and cleanup to ensure memory safety. Always
/// use the provided methods to create and manage threads.
pub struct Thread {
    id: ThreadId,
}

#[repr(u32)]
pub enum ThreadStackSize {
    Default = bindings::TSTACK_DEFAULT,
    _4KB = bindings::TSTACK_4KB,
    _1MB = bindings::TSTACK_1MB,
    _2MB = bindings::TSTACK_2MB,
}

impl Thread {
    /// Create a new Nautilus thread, but do not start it.
    ///
    /// # Arguments
    ///
    /// `function` - A function to be executed by the thread.
    /// `input` - A pointer to the input data for the thread function.
    /// `output` - A pointer to the location where the output of the thread function should be stored.
    /// `detached` - A flag indicating whether the thread should be detached.
    /// `stack_size` - The size of the stack for the new thread.
    ///
    /// # Returns
    ///
    /// A new `Thread` struct.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it calls the C function `nk_thread_create`.
    /// The safety of this function depends on the correct implementation of the C library.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::kernel::thread::{Thread, ThreadStackSize};
    ///
    /// unsafe {
    ///     let thread = Thread::new(None, None, None, false, ThreadStackSize::Default);
    /// }
    /// ```
    pub fn create(
        fun: Option<ThreadFun>,
        input: *mut c_void,
        output: *mut *mut c_void,
        is_detached: bool,
        stack_size: ThreadStackSize,
        bound_cpu: i32,
    ) -> Result<Self> {
        let mut tid = core::ptr::null_mut();
        let mut is_detached_u8: u8;
        if is_detached {
            is_detached_u8 = 1;
        } else {
            is_detached_u8 = 0;
        }

        // SAFETY: `nk_thread_create` is a C function that expects
        // valid function pointers, input pointer, output pointer
        // and stack size for successful thread creation. If successful,
        // it returns a valid pointer in 'tid' that can be used in
        // other thread operations.
        let ret = unsafe {
            bindings::nk_thread_create(
                fun,
                input,
                output,
                is_detached_u8,
                stack_size as u64,
                &mut tid,
                bound_cpu,
            )
        };

        Result::from_error_code(ret).map(|_| Self { id: tid })
    }

    /// Runs the thread.
    ///
    /// # Returns
    ///
    /// Returns an empty `Result` on success, or a `ThreadError` on failure.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it calls the C function `nk_thread_run`.
    /// The safety of this function depends on the correct implementation of the C library.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::kernel::thread::Thread;
    ///
    /// let thread = unsafe { Thread::create(None, None, None, false, ThreadStackSize::Default, -1).unwrap() };
    /// let result = thread.run();
    /// assert!(result.is_ok());
    /// ```
    pub fn run(&self) -> Result {
        // SAFETY: `nk_thread_run` is a C function that expects a valid
        // thread id that was returned from a successful thread creation.
        // If successful, it sets the thread running and returns 0.
        let ret = unsafe { bindings::nk_thread_run(self.id) };

        Result::from_error_code(ret)
    }

    /// Starts and runs a new thread.
    ///
    /// # Arguments
    ///
    /// `function` - A function to be executed by the thread.
    /// `input` - A pointer to the input data for the thread function.
    /// `output` - A pointer to the location where the output of the thread function should be stored.
    /// `detached` - A flag indicating whether the thread should be detached.
    /// `stack_size` - The size of the stack for the new thread.
    /// `bound_cpu` - The cpu to bind the thread to. -1 means no binding.
    ///
    /// # Returns
    ///
    /// A new `Thread` struct.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it calls the C function `nk_thread_start`.
    /// The safety of this function depends on the correct implementation of the C library.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::kernel::thread::{Thread, ThreadStackSize};
    ///
    /// unsafe {
    ///     let thread = Thread::start(None, None, None, false, ThreadStackSize::Default, -1);
    /// }
    /// ```
    pub fn start<F, T>(
        f: F,
        output: *mut *mut c_void,
        is_detached: bool,
        stack_size: ThreadStackSize,
        bound_cpu: i32,
    ) -> Result<Self> 
    where
        F: FnMut() -> T + Send + 'static,
        T: Send + 'static
    {
        let mut tid = core::ptr::null_mut();

        // SAFETY: `nk_thread_start` is a C function that expects valid
        // function pointers, input pointer, output pointer and stack
        // size for successful thread creation and running. If successful,
        // it returns a valid pointer in 'tid' that can be used in other
        // thread operations.
        let ret = unsafe {
            bindings::nk_thread_start(
                Some(call_closure::<F, T>),
                Box::into_raw(Box::new(f)) as *const F as *mut _,
                output,
                is_detached as u8,
                stack_size as u64,
                &mut tid,
                bound_cpu,
            )
        };

        Result::from_error_code(ret).map(|_| Self { id: tid })
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
    pub fn fork(&self) -> ThreadId {
        // SAFETY: `nk_thread_fork` is a C function that forks the current thread.
        // If successful, it returns the thread id of the child to the parent thread, or zero to the child thread.
        unsafe { bindings::nk_thread_fork() }
    }

    /// Set the output for the thread.
    ///
    /// # Arguments
    ///
    /// `result` - A pointer to the result value.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it calls the C function `nk_set_thread_output`.
    /// The safety of this function depends on the correct implementation of the C library.
    pub fn set_output(&self, result: *mut c_void) {
        // SAFETY: `nk_set_thread_output` is a C function that expects a valid pointer for the result value.
        // It sets the output for the thread and does not return a value.
        unsafe { bindings::nk_set_thread_output(result) }
    }

    /// Joins with the thread, waiting for it to terminate.
    ///
    /// # Arguments
    ///
    /// `output` - A pointer to the location where the output of the thread function should be stored.
    ///
    /// # Returns
    ///
    /// Returns an empty `Result` on success, or a `ThreadError` on failure.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it calls the C function `nk_thread_join`.
    /// The safety of this function depends on the correct implementation of the C library.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::kernel::thread::Thread;
    ///
    /// let thread = unsafe { Thread::create(None, None, None, false, ThreadStackSize::Default, -1).unwrap() };
    /// let result = thread.join();
    /// assert!(result.is_ok());
    /// ```
    pub fn join(&self, output: *mut *mut c_void) -> Result {
        // SAFETY: `nk_join` is a C function that expects a valid thread id that was returned from a successful
        // thread creation. It also expects a valid pointer for storing thread output.
        let ret = unsafe { bindings::nk_join(self.id, output) };

        Result::from_error_code(ret)
    }

    /// Destroys the thread.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it calls the C function `nk_thread_destroy`.
    /// The safety of this function depends on the correct implementation of the C library.
    pub fn destroy(self) {
        // SAFETY: `nk_thread_destroy` is a C function that expects a valid thread id that was returned from a successful
        // thread creation. It destroys the thread and does not return a value.
        unsafe { bindings::nk_thread_destroy(self.id) }
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
    pub fn exit(&self, retval: *mut c_void) {
        // SAFETY: `nk_thread_exit` is a C function that expects a valid pointer for the return value.
        // It causes the calling thread to terminate and does not return a value.
        unsafe { bindings::nk_thread_exit(retval) }
    }

    /// Causes the thread to yield execution to another thread that is ready to run.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it calls the C function `nk_thread_yield`.
    /// The safety of this function depends on the correct implementation of the C library.
    pub fn yield_thread() {
        // SAFETY: `nk_yield` is a C function that causes the calling thread to yield execution to another thread that
        // is ready to run and does not return a value.
        unsafe { bindings::nk_yield() }
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
    pub fn name(&self, name: &str) -> Result {
        let cstr = CString::new(name).unwrap();

        // SAFETY: `nk_thread_name` is a C function that expects a valid
        // thread id and a valid null-terminated string pointer for the thread name.
        // If successful, it sets the thread name and returns 0.
        let ret = unsafe { bindings::nk_thread_name(self.id, cstr.as_ptr() as *mut i8) };

        Result::from_error_code(ret)
    }

    /// Gets the thread id of the current thread.
    ///
    /// # Returns
    ///
    /// The thread id of the current thread.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it calls the C function `nk_get_tid`.
    /// The safety of this function depends on the correct implementation of the C library.
    pub fn get_tid() -> Self {
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
        let tid = unsafe { bindings::nk_get_tid() };

        Self { id: tid }
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
    pub fn get_parent_tid() -> Self {
        // SAFETY: `nk_get_parent_tid` is a C function that gets the parent thread id.
        let tid = unsafe { bindings::nk_get_parent_tid() };

        Self { id: tid }
    }
}


