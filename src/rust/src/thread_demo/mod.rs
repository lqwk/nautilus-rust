use crate::prelude::*;
use crate::kernel::thread::{Thread, ThreadStackSize};
use core::ffi::c_void;

fn main() {
    let mut handles = Vec::new();

    for i in 0..5 {
        let handle = Thread::start(
            Some(thread_fun),
            &i as *const i32 as *mut c_void,
            core::ptr::null_mut(),
            false,
            ThreadStackSize::Default,
            -1,
        )
        .unwrap();
        handles.push(handle);
    }

    for handle in handles {
        // Provide a null pointer as the argument for the join method
        handle.join(core::ptr::null_mut()).unwrap();
    }
}

unsafe extern "C" fn thread_fun(arg: *mut c_void, _: *mut *mut c_void) {
    let id = unsafe { *(arg as *mut i32) };
    vc_println!("Thread {} started", id);
    for i in 0..10 {
        vc_println!("Thread {}: {}", id, i);
    }
    vc_println!("Thread {} finished", id);
}



register_shell_command!("rust_thread", "rust_thread", |_| {
    debug!("Entered Rust Threading code.");
    main();
    debug!("Exiting Rust Threading code.");
    0
});