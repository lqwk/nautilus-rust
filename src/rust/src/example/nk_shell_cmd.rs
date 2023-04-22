use core::ffi::{c_char, c_int, c_void};
use crate::kernel::utils::print_to_vc;
use super::nk_rust_example;

// this handler function can be called from the shell after registering it
// unsure whether `buf` and `priv` can be `mut`, keeping `const` to be safe
// nomangle + pub extern "C" means standard C linkage and visibility
#[no_mangle]
pub extern "C" fn example_shell_entry(_buf: *const c_char, _priv_: *const c_void) -> c_int {
    let s = "now entered Rust code\n";
    print_to_vc(s);
    nk_rust_example(8, 1);

    0
}
