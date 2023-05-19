//! Thread Kernel Module
//!
//! This module provides a safe and idiomatic Rust interface for managing AeroKernel threads.
//!
//! The main struct provided by this module is `Thread`. This struct acts as a handle for an
//! AeroKernel thread, and provides methods for creating and controlling threads.
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
use super::bindings;
extern crate cstr_core;
use core::ffi::c_void;

pub type ThreadId = *mut c_void;
pub type ThreadFun = unsafe extern "C" fn(*mut c_void, *mut *mut c_void);

/// A Rust-friendly wrapper for an AeroKernel thread.
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

pub enum ThreadStackSize {
    Default = bindings::TSTACK_DEFAULT as isize,
    _4KB = bindings::TSTACK_4KB as isize,
    _1MB = bindings::TSTACK_1MB as isize,
    _2MB = bindings::TSTACK_2MB as isize,
}

impl Thread {
    /// Create a new AeroKernel thread, but do not start it.
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
    ) -> Result<Self, i32> {
        let mut tid = core::ptr::null_mut();
        let mut is_detached_u8: u8;
        if is_detached {
            is_detached_u8 = 1;
        } else {
            is_detached_u8 = 0;
        }
        
        // SAFETY: `nk_thread_create` is a C function that expects valid function pointers, input pointer, output pointer
        // and stack size for successful thread creation. If successful, it returns a valid pointer in 'tid' that can be used in
        // other thread operations.
        let ret = unsafe {
            bindings::nk_thread_create(fun, input, output, is_detached_u8, stack_size as u64, &mut tid, bound_cpu)
        };
        
        if ret == 0 {
            Ok(Self { id: tid })
        } else {
            Err(ret)
        }
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
    pub fn run(&self) -> Result<(), i32> {
        // SAFETY: `nk_thread_run` is a C function that expects a valid thread id that was returned from a successful
        // thread creation. If successful, it sets the thread running and returns 0.  
        let ret = unsafe { bindings::nk_thread_run(self.id) };
        
        if ret == 0 {
            Ok(())
        } else {
            Err(ret)
        }
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
    pub fn start(
        fun: Option<ThreadFun>,
        input: *mut c_void,
        output: *mut *mut c_void,
        is_detached: bool,
        stack_size: ThreadStackSize,
        bound_cpu: i32,
    ) -> Result<Self, i32> {
        let mut tid = core::ptr::null_mut();
        let mut is_detached_u8: u8;
        if is_detached {
            is_detached_u8 = 1;
        } else {
            is_detached_u8 = 0;
        }
    
        // SAFETY: `nk_thread_start` is a C function that expects valid function pointers, input pointer, output pointer
        // and stack size for successful thread creation and running. If successful, it returns a valid pointer in 'tid' that can be used in
        // other thread operations.
        let ret = unsafe {
            bindings::nk_thread_start(fun, input, output, is_detached_u8, stack_size as u64, &mut tid, bound_cpu)
        };
        
        match ret {
            0 => Ok(Self { id: tid }),
            -1 => Err(-1),  // Could not create thread. handle the -1 case specifically if needed
            _ => Err(ret),
        }
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
    pub fn join(&self, output: *mut *mut c_void) -> Result<(), i32> {
        // SAFETY: `nk_join` is a C function that expects a valid thread id that was returned from a successful
        // thread creation. It also expects a valid pointer for storing thread output.
        let ret = unsafe { bindings::nk_join(self.id, output) };
        
        if ret == 0 {
            Ok(())
        } else {
            Err(ret)
        }
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
    pub fn name(&self, name: &str) -> Result<(), i32> {
        let cstr = cstr_core::CString::new(name).unwrap();
    
        // SAFETY: `nk_thread_name` is a C function that expects a valid thread id and a valid null-terminated string pointer for the thread name.
        // If successful, it sets the thread name and returns 0. 
        let ret = unsafe { bindings::nk_thread_name(self.id, cstr.as_ptr() as *mut i8) };
    
        if ret == 0 {
            Ok(())
        } else {
            Err(ret)
        }
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
    #[cfg(not(feature = "LEGION"))]
    pub fn get_tid() -> Self {
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
