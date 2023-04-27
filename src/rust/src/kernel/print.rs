#[allow(unused_macros)]
use super::bindings;
use core::fmt;

// A ZST that wraps nk_vc_print
#[doc(hidden)]
struct _VcWriter;

impl fmt::Write for _VcWriter {
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

/// Prints to the virtual console.
#[macro_export]
macro_rules! vc_print {
    ($($arg:tt)*) => ($crate::kernel::print::_print(format_args!($($arg)*)));
}

/// Prints to the virtual console with an implicit newline.
#[macro_export]
macro_rules! vc_println {
    () => ($crate::kernel::print::vc_print!("\n"));
    ($($arg:tt)*) => ($crate::kernel::print::vc_print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    _VcWriter.write_fmt(args).unwrap();
}

extern "C" {
    fn debug_error_print(s: *mut i8);
}

// A ZST for debug/error printing
#[doc(hidden)]
struct _LogWriter;

impl fmt::Write for _LogWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // Unlike the _VcWriter, we can't easily break up the message if
        // it's too large and repeatedly call the into the C function,
        // since we don't want the "... DEBUG/ERROR: " prefix appearing
        // more than once. So we limit the amount that can be written
        // using a debug print to a fixed SIZE (an allocation is unacceptable
        // as this may be called when the kernel's allocator is unavailable,
        // broken, or out-of-memory).
        //
        // The C code looks like it also truncates (to 1024 chars including
        // the \0, but not including the "... DEBUG/ERROR: " prefix).
        const SIZE: usize = 1024;
        let mut buf: [u8; SIZE] = [0; SIZE];

        // TODO: why are the debug_println/error_println macros able to
        // print the newline even when "s" is truncated? Shouldn't we
        // need a newline in TRUNC? Weirdly it's working just fine
        // now, but why?
        const TRUNC: &str = "...(trunc)";

        if s.len() < SIZE {
            buf[..(s.len())].copy_from_slice(s.as_bytes());
            buf[s.len()] = 0;
        } else {
            // Truncate the message if it's too large.
            let trunc_start = (SIZE - 1) - TRUNC.len();
            buf[..trunc_start].copy_from_slice(s[..trunc_start].as_bytes());
            buf[trunc_start..(SIZE - 1)].copy_from_slice(TRUNC.as_bytes());
            buf[SIZE - 1] = 0;
        };
        // SAFETY: FFI call.
        unsafe {
            debug_error_print(buf.as_ptr() as *mut i8);
        }
        Ok(())
    }
}

#[doc(hidden)]
pub fn _log(args: fmt::Arguments) {
    use core::fmt::Write;
    _LogWriter.write_fmt(args).unwrap();
}

/// Prints a debug message (truncated if excessively long).
#[macro_export]
macro_rules! debug_print {
    ($($arg:tt)*) => {
        $crate::kernel::print::_log(format_args!("CPU %d (%s%s %lu \"%s\"): DEBUG: {}" , format_args!($($arg)*)))
    };
}

/// Prints a debug message (truncated if excessively long), with an implicit newline.
#[macro_export]
macro_rules! debug_println {
    () => ($crate::kernel::print::debug_print!("\n"));
    ($($arg:tt)*) => ($crate::kernel::print::debug_print!("{}\n", format_args!($($arg)*)));
}

/// Prints an error message (truncated if excessively long).
#[macro_export]
macro_rules! error_print {
    ($($arg:tt)*) => {
        $crate::kernel::print::_log(format_args!("CPU %d (%s%s %lu \"%s\"): ERROR: {}" , format_args!($($arg)*)))
    };
}

/// Prints an error message (truncated if excessively long), with an implicit newline.
#[macro_export]
macro_rules! error_println {
    () => ($crate::kernel::print::error_print!("\n"));
    ($($arg:tt)*) => ($crate::kernel::print::error_print!("{}\n", format_args!($($arg)*)));
}

pub(crate) use debug_print;
pub(crate) use debug_println;
pub(crate) use error_print;
pub(crate) use error_println;
pub(crate) use vc_print;
pub(crate) use vc_println;
