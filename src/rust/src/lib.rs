// unstable feature core::ffi
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]
#![feature(alloc_c_string)]
#![feature(core_ffi_c)]
#![feature(core_c_str)]
#![feature(c_size_t)]
// cargo cult
#![feature(lang_items)]
// no stdlib
#![no_std]
// avoid buildins - we want it to use our library
#![no_builtins]

extern crate alloc;
mod example;
mod parport;
pub mod nk_alloc;
pub mod nk_bindings;
pub mod nk_panic;
//pub mod nk_shell_cmd;
pub mod utils;
