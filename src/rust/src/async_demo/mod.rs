use crate::prelude::*;
use crate::task::{Task, executor::Executor};
use crate::task::utils::yield_now;
use crate::kernel::timer;


fn kernel_main() {
    let mut executor = Executor::new();
    executor.spawn(Task::new(example_task_1()));
    executor.spawn(Task::new(example_task_2()));
    executor.spawn(Task::new(example_task_3()));
    executor.run(true); // This will now return once all tasks are done
}

async fn async_number() -> u32 {
    let ns = 5 * 1_000_000_000; // 5 seconds in nanoseconds
    let start = timer::get_realtime(); // get the current time
    while timer::get_realtime() < start + ns {
        yield_now().await; // yield control to allow other tasks to run
    }
    42
}

async fn example_task_1() {
    vc_println!("Hello from example task 1!\nI'm a blocking task to show the timer!\nWaiting 3 seconds...");
    let ns = 3 * 1_000_000_000; // 3 seconds in nanoseconds
    timer::sleep(ns); // sleep for 3 seconds
    vc_println!("Done sleeping!");
    vc_println!("blocking number 1: {}", 8086);
}

async fn example_task_2() {
    vc_println!("Hello from example task 2! Waiting 5 seconds...");
    let number = async_number().await;
    vc_println!("async number 2: {}", number);
}

async fn async_number_3() -> u32 {
    69
}

async fn example_task_3() {
    vc_println!("Hello from example task 3! Number will return now...");
    let number = async_number_3().await;
    vc_println!("async number 3: {}", number);
}

register_shell_command!("rust_async", "rust_async", |_, _| {
    debug!("Entered Rust Async code.");
    kernel_main();
    debug!("Exiting Rust Async code.");
    0
});
