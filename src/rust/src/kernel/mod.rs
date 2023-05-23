extern crate alloc;

use core::panic::PanicInfo;
use crate::kernel::print::vc_println;

#[allow(clippy::all, clippy::undocumented_unsafe_blocks)]
pub mod bindings;
pub mod allocator;
pub mod sync;
pub mod irq;
pub mod chardev;
pub mod shell;
pub mod print;
pub mod error;
pub mod utils;
pub mod timer;

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    vc_println!("{}", info);

    // SAFETY: FFI call.
    //
    // We don't need to pass the panic message here,
    // since we already printed it above.
    unsafe { bindings::panic(&0_i8 as *const i8); }
}
