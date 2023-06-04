use core::ffi::c_int;

/// A [`Result`] with an `c_int` error type.
///
/// To be used as the return type for functions that may fail.
///
/// # Error codes in C and Rust
///
/// In C, it is common that functions indicate success or failure through
/// their return value; modifying or returning extra data through non-`const`
/// pointer parameters. In particular, in the kernel, functions that may fail
/// typically return an `int` that represents a generic error code. We model
/// those as [`c_int`][core::ffi::c_int].
///
/// In Rust, it is idiomatic to model functions that may fail as returning
/// a [`Result`]. Since in the kernel many functions return an error code,
/// [`Result`] is a type alias for a [`core::result::Result`] that uses
/// a [`c_int`][core::ffi::c_int] as its error type.
///
/// Note that even if a function does not return anything when it succeeds,
/// it should still be modeled as returning a `Result`.
pub type Result<T = ()> = core::result::Result<T, c_int>;

pub trait ResultExt {
    fn as_error_code(&self) -> c_int;
    fn from_error_code(code: c_int) -> Result;
}

impl ResultExt for Result {
    fn as_error_code(&self) -> c_int {
        match *self {
            Ok(_) => 0,
            Err(e) => e
        }
    }

    fn from_error_code(code: c_int) -> Result {
        match code {
            0 => Ok(()),
            c => Err(c)
        }
    }
}
