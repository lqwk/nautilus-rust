use alloc::ffi::CString;

/// Takes a `&str` and provides a C-flavored string that can be passed via FFI.
/// Unless followed by a call to `CString::from_raw` on the returned pointer,
/// this function will leak memory.
pub fn to_c_string(s: &str) -> *mut i8 {
    let c_str = CString::new(s).expect("CString::new failed - did you include a nul byte?");
    c_str.into_raw()
}
