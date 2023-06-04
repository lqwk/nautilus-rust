#![feature(alloc_error_handler)]

// This enables the `some_result.inspect_err(...)` function,
// which makes error logging much more ergonomic. This feature
// is not strictly necessary, and we could define this method
// in our own `ResultExt` trait, but this seems likely to be
// stabilized.
#![feature(result_option_inspect)]

// cfg_accessible is SUPER UNSTABLE and its API may be changed
// or removed without warning. But we need it so that we can
// hook into "#define"s from KConfig for conditional compilation.
// Please see https://github.com/rust-lang/rust/issues/64797.
#![feature(cfg_accessible)]

// no stdlib
#![no_std]
#![no_builtins]


// use saner, more strict interpretation of `unsafe fn`
// (ie. ONLY an obligation to the caller, not a carte-
// blanche to discharge unsafe obligations inside)
// See this RFC for explanation and details:
// https://rust-lang.github.io/rfcs/2585-unsafe-block-in-unsafe-fn.html
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

/// A collection of useful imports. Most Rust modules will want
/// to use this prelude.
#[allow(unused_imports)]
mod prelude;

#[deny(missing_debug_implementations)]
#[warn(clippy::undocumented_unsafe_blocks)]
#[allow(unused_macros, dead_code)]
/// Rust API's for the Nautilus kernel.
///
/// This module contains the kernel APIs that have been ported or wrapped for usage by Rust code in
/// the kernel and is shared by all of them. In other words, all the rest of the Rust code in the
/// kernel (e.g. kernel modules written in Rust) depends on core, alloc and this module. If you need
/// a kernel C API that is not ported or wrapped yet here, then do so first instead of bypassing
/// this module.
pub mod kernel;

/// Simple Rust example module.
mod example;

/// Parallel port driver.
mod parport;

/// Cooperative multitasking demo.
mod async_demo;

/// Threading demo.
mod thread_demo;

/// Virtio GPU driver.
#[cfg_accessible(kernel::bindings::NAUT_CONFIG_RUST_VIRTIO_GPU)]
mod virtio_gpu;
