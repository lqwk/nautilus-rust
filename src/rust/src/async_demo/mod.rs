use crate::prelude::*;

use crate::task::{Task, executor::Executor};

use core::arch::x86_64::_rdtsc;

const CPU_FREQUENCY_HZ: u64 = 3_000_000_000; // Adjust this to match your actual CPU frequency


use futures_util::task::AtomicWaker;
static WAKER: AtomicWaker = AtomicWaker::new();


fn kernel_main() -> () {
    let mut executor = Executor::new();
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(example_task_2()));
    executor.run();
}


async fn async_number() -> u32 {
    let start = unsafe { _rdtsc() };
    let end = start + 5 * CPU_FREQUENCY_HZ;
    while unsafe { _rdtsc() } < end {
        // Do nothing
    }
    42
}

async fn example_task() {
    let number = async_number().await;
    vc_println!("async number: {}", number);
}

async fn async_number_2() -> u32 {
    69
}

async fn example_task_2() {
    let number = async_number_2().await;
    vc_println!("async number 2: {}", number);
}

register_shell_command!("rust_async", "rust_async", |_, _| {
    debug!("Entered Rust Async code.");
    kernel_main();
    debug!("Exiting Rust Async code.");
    0
});
