use super::nk_parport_init;
use core::ffi::{c_char, c_int, c_void};

// this handler function can be called from the shell after registering it
// unsure whether `buf` and `priv` can be `mut`, keeping `const` to be safe
// nomangle + pub extern "C" means standard C linkage and visibility
#[no_mangle]
pub extern "C" fn parport_shell_entry(_buf: *const c_char, _priv_: *const c_void) -> c_int {
    nk_parport_init()
}
