use crate::kernel::thread;
use crate::prelude::*;

use crate::kernel::sync::IRQLock;

make_logging_macros!("thread_demo");

fn basic_thread_test(n: usize) -> Result {
    vc_println!("Beginning basic thread test.");

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
        vc_println!("Basic thread test passed!");
        Ok(())
    } else {
        vc_println!("Basic thread test failed!");
        Err(-1)
    }
}

fn builder_thread_test(n: usize) -> Result {
    vc_println!("Beginning thread builder test.");
    vc_println!("Spawning {n} thread(s) to say hello ...");

    let mut handles = Vec::with_capacity(n);
    for i in 0..n {
        handles.push(
            thread::Builder::new()
                .name(alloc::format!("thread {i}"))
                .inherit_vc()
                .spawn(move || {
                    vc_println!("    [{i}] hello");
                })
                .unwrap(),
        );
    }

    for handle in handles.into_iter() {
        handle.join().unwrap();
    }

    vc_println!("Finished thread builder test.");

    Ok(())
}

register_shell_command!("rust_thread", "rust_thread", |_| {
    debug!("Entered Rust threading demo.");

    basic_thread_test(500)?;

    builder_thread_test(1)?;

    debug!("Exiting Rust threading demo.");

    Ok(())
});
