#[allow(unused_macros)]

use super::bindings;
use core::fmt;

/// A ZST that wraps nk_vc_print
struct VcWriter;

impl fmt::Write for VcWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // We've been given a &str, but the C code will expect a 
        // null-terminated char*. We can avoid an allocation here by
        // copying the string chunkwise onto the stack printing
        // it one chunk at a time (null-terminating each chunk).
        let mut buf: [u8; 64] = [0; 64];

        for chunk in s.as_bytes().chunks(63) {
            buf[0..(chunk.len())].copy_from_slice(chunk);
            buf[chunk.len()] = 0;
            // SAFETY: FFI call for nk_vc_printf (which handles
            // synchronization on its end).
            unsafe {
                bindings::nk_vc_print(buf.as_ptr() as *mut i8);
            }
        }

        Ok(())
    }
}

/// Print to the virtual console
#[macro_export]
macro_rules! vc_print {
    ($($arg:tt)*) => ($crate::kernel::print::_print(format_args!($($arg)*)));
}

/// Print to the virtual console with an implicit newline
#[macro_export]
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
