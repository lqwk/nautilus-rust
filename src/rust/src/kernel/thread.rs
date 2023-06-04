//! Threads.
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
//! In this example, the thread's handle is never used, which means that there is no way
//! for the program to learn when the spawned thread completes or otherwise terminates.
//! 
//! To learn when a thread completes, it is necessary to capture the [`JoinHandle`] object
//! that is returned by the call to [`spawn`], which provides a [`join`][JoinHandle::join]
//! method that allows the caller to wait for the completion of the spawned thread:
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
//! A new thread can be configured before it is spawned via the [`Builder`] type,
//! which currently allows you to set the name, stack size, bound CPU, and whether
//! or not the parent's virtual console is inherited for the thread:
//!
//! ```
//! use crate::kernel::thread;
//!
//! thread::Builder::new()
//!     .name("my_thread")
//!     .inherit_vc()
//!     .spawn(move || { vc_println!("Hello, world!"); });
//! ```
//!
//! # Thread-local storage
//!
//! Thread local storage is not currently implemented. It would be nice to have
//! a TLS API similar to the standard library, but using Nautilus' existing
//! thread local storage C functions.

use alloc::{boxed::Box, ffi::CString, string::String};
use core::{ffi::c_void, time::Duration};

use crate::kernel::{bindings, error::{Result, ResultExt}, print::make_logging_macros};

make_logging_macros!("thread");

extern "C" {
    fn _glue_get_cur_thread() -> *mut bindings::nk_thread_t;
}

unsafe extern "C" fn call_closure_inherit_vc<F, T>(raw_input: *mut c_void, _: *mut *mut c_void)
where
    F: FnMut() -> T,
{
    // SAFETY: `_glue_get_cur_thread` returns a valid pointer (since we are executing
    // in a thread). `(*_glue_get_cur_thread()).parent` is also a valid pointer, since
    // this function was called as a result of spawning a non-detached thread (i.e. we
    // have a parent).
    unsafe { (*_glue_get_cur_thread()).vc =  (*(*_glue_get_cur_thread()).parent).vc; }

    // SAFETY: The C caller makes sure that `raw_input` is the pointer we passed
    // when we called `nk_thread_start`. The referrent of that pointer was a
    // `(F, Option<T>)` tuple, so it is safe to dereference it here as such.
    let (callback, output) = unsafe { &mut *(raw_input as *mut (F, Option<T>)) };
    *output = Some(callback());
}

unsafe extern "C" fn call_closure<F, T>(raw_input: *mut c_void, _: *mut *mut c_void)
where
    F: FnMut() -> T,
{
    // SAFETY: The C caller makes sure that `raw_input` is the pointer we passed
    // when we called `nk_thread_start`. The referrent of that pointer was a
    // `(F, Option<T>)` tuple, so it is safe to dereference it here as such.
    let (callback, output) = unsafe { &mut *(raw_input as *mut (F, Option<T>)) };
    *output = Some(callback());
}

pub type ThreadId = bindings::nk_thread_id_t;

/// The possible stack sizes for a thread.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum StackSize {
    _4KB = bindings::TSTACK_4KB,
    _1MB = bindings::TSTACK_1MB,
    _2MB = bindings::TSTACK_2MB,
}

#[must_use = "must eventually spawn the thread"]
#[derive(Debug, Default)]
pub struct Builder {
    name: Option<String>,
    stack_size: Option<StackSize>,
    bound_cpu: Option<i32>,
    inherits_vc: Option<()>, // could be `bool`, but it's more consistent with
                             // the other fields this way.
}

impl Builder {
    /// Generates the base configuration for spawning a thread, from which
    /// configuration methods can be chained.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::kernel::thread;
    ///
    /// let builder = thread::Builder::new()
    ///                               .name("foo")
    ///                               .stack_size(thread::StackSize::_4KB)
    ///                               .bound_cpu(0);
    ///
    /// let handler = builder.spawn(|| {
    ///     // thread code
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    pub fn new() -> Builder {
        Builder { name: None, stack_size: None, bound_cpu: None, inherits_vc: None }
    }

    /// Names the thread-to-be.
    ///
    /// The name should be not contain internal null bytes (`\0`). Names greater than
    /// Nautilus' `MAX_THREAD_NAME - 1` (currenly `32 - 1 == 31`) will be truncated.
    pub fn name(mut self, name: String) -> Builder {
        self.name = Some(name);
        self
    }

    /// Sets the size of the stack for the new thread.
    pub fn stack_size(mut self, size: StackSize) -> Builder {
        self.stack_size = Some(size);
        self
    }

    /// Sets the bound CPU for the new thread.
    pub fn bound_cpu(mut self, cpu: u8) -> Builder {
        self.bound_cpu = Some(cpu as i32);
        self
    }

    /// Makes the thread-to-be inherit the virtual console of its parent.
    pub fn inherit_vc(mut self) -> Builder {
        self.inherits_vc = Some(());
        self
    }


    /// Spawns a new thread by taking ownership of the [`Builder`], and returns a
    /// [`kernel::error::Result`][`Result`] to its [`JoinHandle`].
    ///
    /// The spawned thread may outlive the caller. The join handle can be used to
    /// block on termination of the spawned thread.
    /// 
    /// For a more complete documentation see [`thread::spawn`][spawn]. 
    ///
    /// # Errors
    /// 
    /// Unlike the [`spawn`] free function, this method yields a
    /// [`kernel::error::Result`][`Result`] to capture any failure to create the 
    /// thread at the OS level.
    ///
    /// # Panics
    /// 
    /// Panics if a thread name was set and it contained internal null bytes.
    pub fn spawn<F, T>(self, f: F) -> Result<JoinHandle<F, T>>
    where
        F: FnMut() -> T + Send + 'static,
        T: Send + 'static
    {
        let mut id = core::ptr::null_mut();
        let data = Some(Box::new((f, None)));

        let Builder { name, stack_size, bound_cpu, inherits_vc } = self;

        let thread_fn = if inherits_vc.is_some() {
            call_closure_inherit_vc::<F, T>
        } else {
            call_closure::<F, T>
        };

        // SAFETY: `nk_thread_create` is a C function that expects valid
        // function pointers, input pointer, output pointer and stack
        // size for successful thread creation and running. If successful,
        // it returns a valid pointer in 'tid' that can be used in other
        // thread operations.
        let ret = unsafe {
            bindings::nk_thread_create(
                Some(thread_fn),
                (& **data.as_ref().unwrap()) as *const _ as *mut c_void,
                // Nautilus' `output` handling for threads seems completely
                // broken, and there is no C code using thread output to refer
                // to ...
                //
                // So we cheat by using the input space above for the output data,
                // and the broken output pointer is left null and untouched.
                core::ptr::null_mut(),
                // TODO: Currently all threads have `is_detached == false`,
                // since making detached threads this way is currently broken.
                // (Is it a Nautilus problem or our problem?)
                false as u8,
                stack_size.map(|size| size as u64).unwrap_or(bindings::TSTACK_DEFAULT as u64),
                &mut id,
                bound_cpu.unwrap_or(bindings::CPU_ANY),
            )
        };

        if ret != 0 {
            error!("failed to create thread");
            return Err(ret);
        }
        
        let name_ptr = name
            .map(|n| {
                CString::new(n)
                    .expect("Name cannot contain internal null bytes.")
                    .into_raw()
            })
            .unwrap_or(core::ptr::null_mut());

        if !name_ptr.is_null() {
            // SAFETY: `nk_thread_name` expects a non-null second argument.
            // We have checked for that above. `id` is also a valid `nk_thread_id_t`,
            // since we acquired it as a result of a successful call to
            // `nk_thread_create`.
            unsafe { bindings::nk_thread_name(id, name_ptr); }

            // SAFETY: This call matches the call to `CString::into_raw`
            // above. It is safe to deallocate this memory even though
            // we just handed it off to C, because `nk_thread_name`
            // copies the passed name into its a string of its own (via
            // `strncpy`) before it returns.
            let _ = unsafe { CString::from_raw(name_ptr) };
        }

        // SAFETY: `id` is a valid `nk_thread_id_t`, since we acquired
        // it as a result of a successful call to `nk_thread_create`.
        let retval = unsafe { bindings::nk_thread_run(id) };

        Result::from_error_code(retval)
            .map(|_| JoinHandle { id, data })
            .inspect_err(|e| error!("Failed to run thread. Error code {e}."))
    }

}


/// An owned permission to join on a thread (block on its termination).
/// 
/// when a `JoinHandle` is dropped there is no longer any handle to the
/// thread and no way to `join` on it, and the memory associated with the
/// thread function and its output is forgotten and cannot be deallocated.
/// 
/// This struct is created by the [`thread::spawn`][spawn] function and the
/// [`thread::Builder::spawn`][Builder::spawn] method.
#[derive(Debug)]
pub struct JoinHandle<F, T> {
    id: ThreadId,
    data: Option<Box<(F, Option<T>)>>
}

impl<F, T> JoinHandle<F, T> {
    /// Waits for the associated thread to finish.
    ///
    /// This function will return immediately if the associated thread has already finished.
    pub fn join(mut self) -> Result<T> {
        // SAFETY: `nk_join` is a C function that expects a valid thread id that was returned from a successful
        // thread creation. It also expects a valid pointer for storing thread output.
        let retval = unsafe { bindings::nk_join(self.id, core::ptr::null_mut() as _) };

        Result::from_error_code(retval)
            .map(|_| { self.data.take().unwrap().1.unwrap() })
            .inspect_err(|e| error!("Failed to join on thread. Error code {e}."))
    }

}

impl<F, T> Drop for JoinHandle<F, T> {
    fn drop(&mut self) {
        // TODO: should we manually destroy the thread here,
        // or let them be reaped by the scheduler?

        let data = self.data.take();

        if let Some(thread_mem) = data {
            // If the `JoinHandle` was dropped with the inner data still
            // alive, then the thread was never joined-on, and may still
            // be running. Running a destructor on memory it's using
            // would be UB, so we have to leak it here.
            Box::leak(thread_mem);
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
/// This call will create a thread using default parameters of [`Builder`], if you want to specify the
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
///
/// # Panics
///
/// Panics if Nautilus fails to create a thread; use [`Builder::spawn`] to recover from such errors.
pub fn spawn<F, T>(f: F) -> JoinHandle<F, T>
where
    F: FnMut() -> T + Send + 'static,
    T: Send + 'static
{ 
    Builder::new().spawn(f).expect("Thread failed to spawn!")
}


/// Causes the thread to yield execution to another thread that is ready to run.
pub fn thread_yield() {
    // SAFETY: `nk_yield` is a C function that causes the calling thread
    // to yield execution to another thread that is ready to run and
    // does not return a value.
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
pub unsafe fn exit(retval: *mut c_void) {
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

/// Puts the current thread to sleep for the specified amount of time.
///
/// This function is blocking, and should not be used in `async` functions.
///
/// # Examples
///
/// ```
/// use core::time;
/// use crate::kernel::thread;
///
/// let ten_millis = time::Duration::from_millis(10);
/// let now = time::Instant::now();
///
/// thread::sleep(ten_millis);
///
/// assert!(now.elapsed() >= ten_millis);
/// ```
pub fn sleep(dur: Duration) {
    // SAFETY: FFI call.
    unsafe { bindings::nk_sleep(dur.as_nanos() as u64); }
}
