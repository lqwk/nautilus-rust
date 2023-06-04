use core::sync::atomic::{AtomicUsize, Ordering};

use crate::kernel::sync::Spinlock;
use crate::kernel::thread;
use crate::prelude::*;

make_logging_macros!("thread_demo");

fn basic_thread_test(n: usize) -> Result {
    let mut handles = Vec::with_capacity(n);

    // Let's spawn `n` threads and have each of them increment a
    // counter 10000 times. Both the wrapper around Nautilus' spinlock
    // and Rust's core atomics would work here--let's try both.
    let lock_total = Arc::new(Spinlock::new(0_usize));
    let atomic_total = Arc::new(AtomicUsize::new(0));

    for _ in 0..n {
        let my_lock_total = Arc::clone(&lock_total);
        let my_atomic_total = Arc::clone(&atomic_total);
        handles.push(thread::spawn(move || {
            for _ in 0..10000 {
                *my_lock_total.lock() += 1;
                my_atomic_total.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    for handle in handles.into_iter() {
        handle
            .join()
            .inspect_err(|_| error!("failed to join on thread."))?;
    }

    let expected = n * 10000;
    let observed_lock = *lock_total.lock();
    let observed_atomic = atomic_total.load(Ordering::SeqCst);

    if observed_lock == expected && observed_atomic == expected {
        return Ok(());
    }

    if observed_lock != expected {
        vc_println!(
            "Unexpected spinlock total ({} != {}).",
            observed_lock,
            n * 10000
        )
    }
    if observed_atomic != expected {
        vc_println!(
            "Unexpected atomic total ({} != {}).",
            observed_atomic,
            n * 10000
        )
    }

    Err(-1)
}

fn builder_thread_test(n: usize) -> Result {
    vc_println!("Spawning {n} thread(s) to sleep and say hello ...");

    let mut handles = Vec::with_capacity(n);
    for i in 0..n {
        handles.push(
            thread::Builder::new()
                .name(alloc::format!("thread {i}"))
                .inherit_vc()
                .spawn(move || {
                    thread::sleep(core::time::Duration::from_secs(i as u64));
                    vc_println!("    [{i}] hello");
                    i
                })?,
        );
    }

    // We had each thread return `i` above. Let's make sure we receive those
    // outputs correctly when we join on them.
    let mut fail = false;
    for (i, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(output) => {
                if output != i {
                    vc_println!("Wrong output from thread {} ({} != {}).", i, output, i);
                    fail = true;
                }
            }, 
            Err(_) => {
                vc_println!("Failed to join on thread {i}.");
                fail = true;
            }
        }
    }

    (!fail).then_some(()).ok_or(-1)

}

register_shell_command!("rust_thread", "rust_thread", |_| {
    vc_println!("Entered Rust threading demo.");

    basic_thread_test(500)
        .inspect(|_| vc_println!("Initial test passed."))
        .and_then(|_| builder_thread_test(10))
        .inspect_err(|_| vc_println!("Rust threading demo failed!"))?;

    vc_println!("Exiting Rust threading demo.");

    Ok(())
});
