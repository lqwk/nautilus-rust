use core::ffi::{c_char, c_int, c_void};

use crate::{
    example::example::nk_rust_example,
    nk_bindings,
    utils::{print_to_vc, to_c_string},
};

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

pub fn register_nk_shell_cmd(
    func: extern "C" fn(*mut i8, *mut c_void) -> c_int,
    name: &str,
    help: &str,
) {
    let cmd_c = to_c_string(name);
    let help_c = to_c_string(help);
    let _cmd_impl = nk_bindings::shell_cmd_impl {
        cmd: cmd_c,
        help_str: help_c,
        handler: Some(func),
    };
    // TODO: perhaps create an internal NK API (not the current macro hack)
    // for registering shell commands at compile or run time
    //unsafe {nk_register_new_shell_cmd...}
    todo!("incomplete bridge to NK APIs here");
    // don't forget to take back `cmd_c` and `help_c` with `CString::from_raw`...
}
