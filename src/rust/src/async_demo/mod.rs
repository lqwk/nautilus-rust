use crate::prelude::*;
use crate::kernel::task::{Task, executor::Executor};
use crate::kernel::task::utils::yield_now;
use crate::kernel::timer;
use rand_core::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

make_logging_macros!("async");

fn kernel_main() {
    let mut executor = Executor::new();

    vc_println!("Spawning 100 async tasks...");

    // Spawn 100 async tasks
    for i in 0..99 {
        let duration_secs = generate_random_float(i);
        executor.spawn(Task::new(async_task(i, duration_secs)));
    }


    executor.run(true); // This will now return once all tasks are done
}

// Generate a random floating point number
// Hack to get around the fact rand doesn't work in no_std
fn generate_random_float(seed: u64) -> f64 {
    let mut rng = XorShiftRng::seed_from_u64(seed);
    let float_part: f64 = rng.next_u32() as f64 / u32::MAX as f64;  // generates a random float between 0 and 1
    float_part * 10.0  // scales it to a range of 0.0 to 10.0
}

async fn async_number(task_num: u64, duration: f64) -> u32 {
    let ns = (duration * 1_000_000_000.0) as u64; // converts duration to nanoseconds
    let start = timer::get_realtime();
    while timer::get_realtime() < start + ns {
        yield_now().await;
    }
    task_num as u32 // return the simulated result of the async task
}

// This is the async task that will be spawned
async fn async_task(task_num: u64, duration: f64) {
    vc_println!("Hello from async task {}! Waiting {} seconds...", task_num, duration);
    let num = async_number(task_num, duration).await;
    vc_println!("Async task {} done!", num);
}

register_shell_command!("rust_async", "rust_async", |_| {
    debug!("Entered Rust Async code.");
    kernel_main();
    debug!("Exiting Rust Async code.");
    Ok(())
});
