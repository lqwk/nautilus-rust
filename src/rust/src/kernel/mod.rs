extern crate alloc;

use core::cmp::min;
use core::panic::PanicInfo;

pub mod bindings;
pub mod allocator;
pub mod utils;

// #[cfg(not(test))]
#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    let panic_msg = match info.message() {
        Some(m) => match m.as_str() {
            Some(s) if !s.contains('\0') => m
                .as_str()
                .unwrap_or("Panic occurred in Rust (unable to format panic message)."),
            _ => "Panic occurred in Rust (invalid panic message).",
        },
        None => "Panic occurred in Rust.",
    }
    .as_bytes();

    // we should not allocate memory here, since this function might be
    // called as a result of a failure to allocate memory.
    //
    // make a buffer of known size, copy the message, and add text
    // indicating the message was truncated if it's too long.
    let mut msg_buf: [u8; 8192] = [0; 8192];
    let copy_len = min(panic_msg.len(), msg_buf.len());
    msg_buf[..copy_len].clone_from_slice(&panic_msg[..copy_len]);
    // all bytes after the message - a minimum of 1 - should be 0x0 (nul terminator).
    if panic_msg.len() >= msg_buf.len() {
        let truncation_msg = "...(trunc)\0".as_bytes();
        let mlen = msg_buf.len();
        msg_buf[(mlen - truncation_msg.len())..].copy_from_slice(truncation_msg);
    }

    let buf_ptr = msg_buf.as_ptr() as *const i8;
    unsafe {
        // this is fine because this function never returns;
        // it might not be okay otherwise
        bindings::panic(buf_ptr);
    }
}
