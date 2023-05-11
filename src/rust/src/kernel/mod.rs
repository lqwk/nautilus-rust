extern crate alloc;

use core::panic::PanicInfo;
use crate::vc_println;

pub mod bindings;
pub mod allocator;
pub mod irq;
pub mod shell;
pub mod print;
pub mod error;
pub mod utils;

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    vc_println!("{}", info);
    unsafe {
        // SAFETY: FFI call.
        //
        // We don't need to pass the panic message here,
        // since we already printed it above.
        bindings::panic(&0_i8 as *const i8);
    }
}