#[allow(unused_macros)]

macro_rules! vc_print {
    ($($arg:tt)*) => {
        let mut s = alloc::format!($($arg)*);
        let c_str = $crate::kernel::utils::to_c_string(&s);

        // SAFETY: Just an FFI call.
        unsafe { $crate::kernel::bindings::nk_vc_print(c_str); }

        // SAFETY: We free the memory that we allocated for `c_str`.
        unsafe { _ = alloc::ffi::CString::from_raw(c_str); }
    }
}

macro_rules! vc_println {
    () => ($crate::kernel::print::vc_print!("\n"));
    ($($arg:tt)*) => {
        let mut s = alloc::format!($($arg)*);
        s.push('\n'); // TODO: How to avoid this potential extra allocation?
                      //       Also, can we avoid allocations altogether if
                      //       the arguments to `format` are string literals?
        let c_str = $crate::kernel::utils::to_c_string(&s);

        // SAFETY: Just an FFI call.
        unsafe { $crate::kernel::bindings::nk_vc_print(c_str); }

        // SAFETY: We free the memory that we allocated for `c_str`.
        unsafe { _ = alloc::ffi::CString::from_raw(c_str); }

    }
}

pub(crate) use vc_print;
pub(crate) use vc_println;
