use crate::prelude::*;
use crate::kernel::thread;

use crate::kernel::sync::IRQLock;

make_logging_macros!("thread_demo");

register_shell_command!("rust_thread", "rust_thread", |_| {
    debug!("Entered Rust Threading code.");

    const N: usize = 500;

    let mut handles = Vec::with_capacity(N);
    let total = Arc::new(IRQLock::new(0_usize));

    for _ in 0..N {
        let my_total = Arc::clone(&total);
        handles.push(thread::spawn(
            move || {
                let mut value = my_total.lock();
                *value += 1;
            },
            false,
            thread::StackSize::Default,
            1,
        ));
    }

    for handle in handles.into_iter() {
        handle.join().unwrap();
    }

    debug!("expected total is: {N}. observed total is {}.", total.lock());

    Ok(())
});
