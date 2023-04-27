#![feature(panic_info_message)]
#![feature(alloc_error_handler)]
#![feature(c_size_t)]
#![feature(lang_items)]


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

mod prelude;
mod kernel;
mod example;
mod parport;
