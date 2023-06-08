use crate::prelude::*;
use crate::kernel::task::{Task, executor::Executor};
use crate::kernel::task::utils::yield_now;
use crate::kernel::timer;
use core::time::Duration;

make_logging_macros!("async_demo");

fn kernel_main() {
    let mut executor = Executor::new();

    vc_println!("Spawning 100 async tasks...");

    // Spawn 100 async tasks
    for i in 0..99 {
        let dur = Duration::from_secs_f32(generate_random_f32(0.0, 10.0));
        executor.spawn(Task::new(async_task(i, dur)));
    }


    executor.run(true); // This will now return once all tasks are done
}

/// Generate a random floating point number
///
/// Eventually this should be brought into a `rand` module
/// (with an API similar to the standard lib probably).
fn generate_random_f32(min: f32, max: f32) -> f32 {
    // SAFETY: FFI call.
    let scale = unsafe { 
        (super::kernel::bindings::rand() as f32) / (super::kernel::bindings::RAND_MAX as f32)
    };
    min + scale * ( max - min )
}

async fn async_number(task_num: u64, dur: Duration) -> u32 {
    let ns = dur.as_nanos() as u64;
    let start = timer::get_realtime();
    while timer::get_realtime() < start + ns {
        yield_now().await;
    }
    task_num as u32 // return the simulated result of the async task
}

// This is the async task that will be spawned
async fn async_task(task_num: u64, dur: Duration) {
    vc_println!("Hello from async task {}! Waiting for {} secs...", task_num, dur.as_secs_f32());
    let num = async_number(task_num, dur).await;
    vc_println!("Async task {} done!", num);
}

register_shell_command!("rust_async", "rust_async", |_| {
    debug!("Entered Rust Async code.");
    kernel_main();
    debug!("Exiting Rust Async code.");
    Ok(())
});
