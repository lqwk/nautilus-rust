# Task Crate

## Overview

This crate provides a light-weight, efficient task execution system for Rust programs. It features a task queue, executor, and utilities for yielding execution. It is designed for use in environments without a standard Rust runtime, such as `no_std` environments or bare metal systems.

## Modules

The crate is composed of several modules, each with their own functionality:

- `executor.rs`: This module provides an `Executor` struct which is used to manage and execute tasks. It uses a `BTreeMap` to store tasks and an `ArrayQueue` to manage the execution order of tasks.

- `simple_executor.rs`: This module provides a `SimpleExecutor` struct, which is a simplified version of the Executor. It uses a `VecDeque` to manage tasks, providing a simpler but less feature-rich alternative to the Executor.

- `utils.rs`: This module provides utility functions for task management, including `yield_now` which allows a task to voluntarily yield its execution slot to other tasks.

- `mod.rs`: This is the main module of the crate. It defines the `Task` struct which represents a task to be executed, and the `TaskId` struct which is used to uniquely identify tasks.

## Usage

To use the task execution system, you should create an instance of `Executor` or `SimpleExecutor`, then spawn tasks into it. Tasks are represented by the `Task` struct, and can be created from any type that implements the `Future` trait.

### A basic example:

```rust
let mut executor = Executor::new();

let task = Task::new(async {
    // Task code goes here
});

executor.spawn(task);
executor.run();
```

### Usage with Yielding:

This crate also provides a utility function `yield_now` for voluntarily yielding execution from the current task. This can be used to avoid monopolizing the executor when a task has nothing more to do at the moment. Here is an example of its use:

```rust
let mut executor = Executor::new();

let task1 = Task::new(async {
    // Some work here...
    utils::yield_now().await;
    // The task will resume here after yielding execution
    // More work...
});

let task2 = Task::new(async {
    // This task can run while task1 is yielded
});

executor.spawn(task1);
executor.spawn(task2);
executor.run();
```
In this example, `task1` does some work, then yields execution using `yield_now`. The executor then has the opportunity to run other tasks (like `task2` in this case) before `task1` resumes.

Please note that `yield_now` is a cooperative mechanism: a task must choose to call it to yield execution. Tasks that do not call `yield_now` will not yield execution to other tasks, and can monopolize the executor if they run for a long time without completing.

## Requirements

This crate is designed for use in `no_std` environments, and as such does not use the Rust standard library. It does, however, require the `alloc` crate for dynamic memory allocation. Be aware that this crate may not be suitable for use in all `no_std` environments, particularly those without a memory allocator.

## Limitations

This task execution system is quite basic and may not be suitable for all use cases. It does not support task prioritization, preemption, or many other features found in more sophisticated task scheduling systems.

## License

This crate is distributed under the terms of the MIT license.

## Acknowledgments

This crate contains code from the project "Writing an OS in Rust" by Philipp Oppermann. The original project can be found at [github.com/phil-opp/blog_os](https://github.com/phil-opp/blog_os). We are grateful to Philipp Oppermann for his work.

Copyright (c) 2019 Philipp Oppermann