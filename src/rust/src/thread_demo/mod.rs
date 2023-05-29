use crate::kernel::thread;
use crate::prelude::*;

use crate::kernel::sync::IRQLock;

make_logging_macros!("thread_demo");

fn basic_thread_test(n: usize) -> Result {
    debug!("Beginning basic thread test.");

    let mut handles = Vec::with_capacity(n);
    let total = Arc::new(IRQLock::new(0_usize));

    for _ in 0..n {
        let my_total = Arc::clone(&total);
        handles.push(thread::spawn(move || {
            let mut value = my_total.lock();
            *value += 1;
        }));
    }

    for handle in handles.into_iter() {
        handle.join().unwrap();
    }

    if *total.lock() == n {
        debug!("Basic thread test passed!");
        Ok(())
    } else {
        debug!("Basic thread test failed!");
        Err(-1)
    }
}

fn builder_thread_test(n: usize) -> Result {
    let mut handles = Vec::with_capacity(n);

    for i in 0..n {
        handles.push(
            thread::Builder::new()
                .name(alloc::format!("thread {i}"))
                .spawn(move || {
                    debug!("[{i}] hello");
                })
                .unwrap(),
        );
    }

    for handle in handles.into_iter() {
        handle.join().unwrap();
    }

    Ok(())
}

register_shell_command!("rust_thread", "rust_thread", |_| {
    debug!("Entered Rust Threading code.");

    const N: usize = 500;
    basic_thread_test(N)?;

    builder_thread_test(N)?;

    Ok(())
});
