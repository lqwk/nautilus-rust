/// Register a shell command
///
/// # Arguments
///
/// `cmd`     - A string literal for the name of the command in the shell
/// `help`    - A string literal for the help message
/// `handler` - The function to be run when the command is executed, taking
/// two arguments: a *mut i8 buffer and a *mut c_void buffer. Consult the
/// relevant C code for more information.
///
/// # Examples
///
/// ```
/// use crate::kernel::{shell::register_shell_command, print::vc_println};
///
/// register_shell_command!("sayhello", "sayhello", |_, _| {
///     vc_println!("hello");
/// });
/// ```
macro_rules! register_shell_command {
    ($cmd:expr, $help:expr, $handler:expr) => {
        // Rust macros can't create new identifiers programatically as easily
        // as C can. We use paste to do this.
        paste::paste! {
            extern "C" fn [<handle_ $cmd>](buf: *mut i8, _priv: *mut core::ffi::c_void) -> i32 {
                // SAFETY: The C code invoking this handler will ensure that `buf` points
                // to a valid null-terminated C char*. If this condition is violated, it
                // is a bug in Nautilus' shell and not our fault.
                let command = unsafe { core::ffi::CStr::from_ptr(buf) };

                $handler(command.to_str().expect("Shell command string is not valid UTF-8."))
            }

            // Nautilus shell commands are registered by placing a pointer in the
            // the ".shell_cmds" section of the binary.
            #[no_mangle]
            #[link_section = ".shell_cmds"]
            static mut [<_nk_cmd_ $cmd>]:
                *const $crate::kernel::bindings::shell_cmd_impl = &$crate::kernel::bindings::shell_cmd_impl {
                    cmd: concat!($cmd, "\0").as_ptr() as *mut i8,
                    help_str: concat!($help, "\0").as_ptr() as *mut i8,
                    handler: Some([<handle_ $cmd>]),
            } as *const _ ;
        }
    };
}

pub(crate) use register_shell_command;
