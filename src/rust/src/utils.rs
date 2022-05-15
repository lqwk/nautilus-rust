use alloc::ffi::CString;

use crate::nk_bindings;

/// Takes a `&str` and provides a C-flavored string that can be passed via FFI.
/// Unless followed by a call to `CString::from_raw` on the returned pointer,
/// this function will leak memory.
pub fn to_c_string(s: &str) -> *mut i8 {
    let c_str = CString::new(s).expect("CString::new failed - did you include a nul byte?");
    c_str.into_raw()
}

pub fn print_to_vc(s: &str) {
    let c_str = to_c_string(s);
    unsafe {
        // c_str is safe to pass to nk_vc_printf;
        // it is a nul-terminated C string.
        nk_bindings::nk_vc_printf(c_str);
        // nk_vc_printf obeys the invariant required for `from_raw`
        // (it does not mutate or free the string).
        // We are free to "take back" the memory associated with the string.
        _ = CString::from_raw(c_str);
        // memory allocated for the string should be freed here.
    }
}
