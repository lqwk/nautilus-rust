pub use alloc::{
    vec,
    vec::Vec,
    boxed::Box,
    string::String,
    sync::Arc
};

pub(crate) use crate::kernel::shell::register_shell_command;
pub(crate) use crate::kernel::print::{vc_print, vc_println, make_logging_macros};

pub use crate::kernel::error::{Result, ResultExt};
