use crate::prelude::*;
use crate::kernel::thread::{Thread, ThreadStackSize};

make_logging_macros!("thread_demo");

register_shell_command!("rust_thread", "rust_thread", |_| {
    debug!("Entered Rust Threading code.");

    let mut v = Vec::new();
    let _ = Thread::start(
        move || {
            debug!("I've been called.");
            for i in 0..10 {
                v.push(i);
                debug!("{v:?}");
            }
            loop { };
        },
        core::ptr::null_mut(),
        false,
        ThreadStackSize::Default,
        1,
    );

    debug!("Exiting Rust Threading code.");
    Ok(())
});
