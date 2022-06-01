// unstable feature core::ffi
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]
#![feature(alloc_c_string)]
#![feature(core_ffi_c)]
#![feature(core_c_str)]
#![feature(c_size_t)]
#![feature(lang_items)]
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
mod example;
mod parport;
pub mod nk_alloc;
pub mod nk_bindings;
pub mod nk_panic;
//pub mod nk_shell_cmd;
pub mod utils;
