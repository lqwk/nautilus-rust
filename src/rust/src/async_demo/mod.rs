use crate::prelude::*;

use crate::task::{Task, simple_executor::SimpleExecutor};

fn kernel_main() -> () {
    let mut executor = SimpleExecutor::new();
    executor.spawn(Task::new(example_task()));
    executor.run();
}


// Below is the example_task function again so that you don't have to scroll up

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    vc_println!("async number: {}", number);
}

register_shell_command!("rust_async", "rust_async", |_, _| {
    debug!("Entered Rust Async code.");
    kernel_main();
    debug!("Exiting Rust Async code.");
    0
});
