use crate::kernel::thread;
use crate::prelude::*;

use crate::kernel::sync::IRQLock;

make_logging_macros!("thread_demo");

fn basic_thread_test(n: usize) -> Result {
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
        handle
            .join()
            .inspect_err(|_| error!("failed to join on thread."))?;
    }

    if *total.lock() == n {
        Ok(())
    } else {
        Err(-1)
    }
}

fn builder_thread_test(n: usize) -> Result {
    vc_println!("Spawning {n} thread(s) to say hello ...");

    let mut handles = Vec::with_capacity(n);
    for i in 0..n {
        handles.push(
            thread::Builder::new()
                .name(alloc::format!("thread {i}"))
                .inherit_vc()
                .spawn(move || {
                    thread::sleep(core::time::Duration::from_secs(i as u64));
                    vc_println!("    [{i}] hello");
                })?,
        );
    }

    for handle in handles.into_iter() {
        handle.join()?;
    }

    Ok(())
}

register_shell_command!("rust_thread", "rust_thread", |_| {
    vc_println!("Entered Rust threading demo.");

    basic_thread_test(500)
        .and_then(|_| builder_thread_test(10))
        .inspect_err(|_| vc_println!("Rust threading demo failed!"))?;

    vc_println!("Exiting Rust threading demo.");

    Ok(())
});
