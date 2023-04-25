#[allow(unused_macros)]

use super::{bindings, utils::to_c_string};
use alloc::ffi::CString;
use core::fmt;

/// A ZST that wraps nk_vc_print
struct VcWriter;

impl fmt::Write for VcWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // TODO: We currently have to allocate a new nul-terminated
        // string for FFI. This is a little awkward. But it's also
        // not ergonomic to require consumers of Rust APIs to use
        // core::ffi::CString anytime they want a string that could
        // possibly end up on the C side of things. It would be 
        // nice if there were C string literals (e.g. c"hello").
        // Maybe we could provide a c_str! macro?
        let c_str = to_c_string(&s);

        // SAFETY: FFI call for nk_vc_printf (which handles
        // synchronization on its end). We also explicitly
        // deallocate the string behind the pointer previously
        // allocated and temporarily leaked in `to_c_string`.
        unsafe {
            bindings::nk_vc_print(c_str);
            _ = CString::from_raw(c_str);
        }
        Ok(())
    }
}

/// Print to the virtual console
macro_rules! vc_print {
    ($($arg:tt)*) => ($crate::kernel::print::_print(format_args!($($arg)*)));
}

/// Print to the virtual console with an implicit newline
macro_rules! vc_println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::kernel::print::vc_print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    VcWriter.write_fmt(args).unwrap();
}

pub(crate) use vc_print;
pub(crate) use vc_println;
