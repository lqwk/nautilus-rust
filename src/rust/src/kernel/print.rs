use super::bindings;
use core::fmt;

// A ZST that wraps nk_vc_print
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
macro_rules! vc_print {
    ($($arg:tt)*) => ($crate::kernel::print::_print(format_args!($($arg)*)));
}

/// Prints to the virtual console with an implicit newline.
macro_rules! vc_println {
    () => ($crate::vc_print!("\n"));
    ($($arg:tt)*) => ($crate::kernel::print::vc_print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    _VcWriter.write_fmt(args).unwrap();
}

extern "C" {
    fn _glue_log_print(s: *mut i8);
}

// A ZST for debug/error printing
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

        // TODO: why are the debug/error/info/warn macros able to
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
            _glue_log_print(buf.as_ptr() as *mut i8);
        }
        Ok(())
    }
}

#[doc(hidden)]
pub fn _log(_args: fmt::Arguments) {
    use core::fmt::Write;
    _LogWriter.write_fmt(_args).unwrap();
}

macro_rules! debug_print {
    ($($arg:tt)*) => {{
        #[cfg_accessible($crate::kernel::bindings::NAUT_CONFIG_DEBUG_RUST)]
        $crate::kernel::print::_log(format_args!("CPU %d (%s%s %lu \"%s\"): DEBUG: {}\n",
                                    format_args!($($arg)*)));
    }};
}

macro_rules! error_print {
    ($($arg:tt)*) => {{
        $crate::kernel::print::_log(format_args!("CPU %d (%s%s %lu \"%s\"): ERROR at src/rust/{}({}): {}\n",
                                    core::file!(), core::line!(), format_args!($($arg)*)));
    }};
}

macro_rules! warn_print {
    ($($arg:tt)*) => {{
        $crate::kernel::print::_log(format_args!("CPU %d (%s%s %lu \"%s\"): WARNING: {}\n",
                                    format_args!($($arg)*)));
    }};
}

macro_rules! info_print {
    ($($arg:tt)*) => {{
        $crate::kernel::print::_log(format_args!("CPU %d (%s%s %lu \"%s\"): {}\n",
                                    format_args!($($arg)*)));
    }};
}

// Magic needed for certain macro-generating macros. Rust temporarily had this feature
// in 1.63 through the `$$` metavariable, but the feature was reverted soon after.
// We could use `#![feature(macro_metavar_expr)]`, but in an effort to avoid unstable
// features, we use this hack.
//
// See https://github.com/rust-lang/rust/issues/35853
// and https://github.com/rust-lang/rust/issues/83527.
macro_rules! with_dollar_sign {
    ($($body:tt)*) => {
        macro_rules! __with_dollar_sign { $($body)* }
        __with_dollar_sign!($);
    }
}

/// Makes the `debug`, `error`, `warn`, and `info` macros using the given prefix.
///
/// ```
/// make_logging_macros!("example");
/// ```
///
/// is analogous to the C code:
///
/// ```
/// #define DEBUG(fmt, args...) DEBUG_PRINT("example: " fmt, ##args)
/// #define ERROR(fmt, args...) ERROR_PRINT("example: " fmt, ##args)
/// #define WARN(fmt, args...)  WARN_PRINT("example: " fmt, ##args)
/// #define INFO(fmt, args...)  INFO_PRINT("example: " fmt, ##args)
/// ```
macro_rules! make_logging_macros {
    ($prefix:expr) => {
        $crate::kernel::print::with_dollar_sign! {
            ($d:tt) => {
                /// Logs a debug message (truncated if excessively long).
                /// This macro is a noop if Rust debug prints are disabled in Kconfig.
                #[allow(unused_macros)]
                macro_rules! debug {
                    ($d($d args:expr),*) => {{
                        $crate::kernel::print::debug_print!("{}: {}", $prefix, format_args!($d($d args),*));
                    }};
                }
            }
        }

        $crate::kernel::print::with_dollar_sign! {
            ($d:tt) => {
                /// Logs an error message (truncated if excessively long).
                #[allow(unused_macros)]
                macro_rules! error {
                    ($d($d args:expr),*) => {{
                        $crate::kernel::print::error_print!("{}: {}", $prefix, format_args!($d($d args),*));
                    }};
                }
            }
        }

        $crate::kernel::print::with_dollar_sign! {
            ($d:tt) => {
                /// Logs a warning message (truncated if excessively long).
                #[allow(unused_macros)]
                macro_rules! warn {
                    ($d($d args:expr),*) => {{
                        $crate::kernel::print::warn_print!("{}: {}", $prefix, format_args!($d($d args),*));
                    }};
                }
            }
        }

        $crate::kernel::print::with_dollar_sign! {
            ($d:tt) => {
                /// Logs an info message (truncated if excessively long).
                #[allow(unused_macros)]
                macro_rules! info {
                    ($d($d args:expr),*) => {{
                        $crate::kernel::print::info_print!("{}: {}", $prefix, format_args!($d($d args),*));
                    }};
                }
            }
        }
    };

    ($prefix:expr, $config:ident) => {
        $crate::kernel::print::with_dollar_sign! {
            ($d:tt) => {
                /// Logs a debug message (truncated if excessively long).
                /// This macro is a noop if the relevant setting (the second argument
                /// passed to `make_logging_macros!`) is disabled in Kconfig.
                #[allow(unused_macros)]
                macro_rules! debug {
                    ($d($d args:expr),*) => {{
                        #[cfg_accessible($crate::kernel::bindings::$config)]
                        $crate::kernel::print::debug_print!("{}: {}", $prefix, format_args!($d($d args),*));
                    }};
                }
            }
        }

        $crate::kernel::print::with_dollar_sign! {
            ($d:tt) => {
                /// Logs an error message (truncated if excessively long).
                #[allow(unused_macros)]
                macro_rules! error {
                    ($d($d args:expr),*) => {{
                        $crate::kernel::print::error_print!("{}: {}", $prefix, format_args!($d($d args),*));
                    }};
                }
            }
        }

        $crate::kernel::print::with_dollar_sign! {
            ($d:tt) => {
                /// Logs a warning message (truncated if excessively long).
                #[allow(unused_macros)]
                macro_rules! warn {
                    ($d($d args:expr),*) => {{
                        $crate::kernel::print::warn_print!("{}: {}", $prefix, format_args!($d($d args),*));
                    }};
                }
            }
        }

        $crate::kernel::print::with_dollar_sign! {
            ($d:tt) => {
                /// Logs an info message (truncated if excessively long).
                #[allow(unused_macros)]
                macro_rules! info {
                    ($d($d args:expr),*) => {{
                        $crate::kernel::print::info_print!("{}: {}", $prefix, format_args!($d($d args),*));
                    }};
                }
            }
        }
    };
}

#[allow(unused_imports)]
pub(crate) use {vc_print, vc_println, debug_print, error_print, warn_print, info_print, make_logging_macros, with_dollar_sign};
