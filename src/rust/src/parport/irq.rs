use core::ffi::{c_int, c_void};

use crate::nk_bindings;

use super::Parport;

pub struct Irq {
    num: u16,
}

impl Irq {
    pub unsafe fn new(num: u16) -> Self {
        Irq {
            num: num
        }
    }

    unsafe fn register_irq_handler(num: u16, parport: &Parport) {
        let handler = interrupt_handler;
        let parport: *const Parport = parport;
        unsafe {
            // `Parport` reference is not actually mutated in C code
            // (in fact, it's completely opaque)
            nk_bindings::register_irq_handler(num, Some(handler), parport as *mut c_void);
        }
        // check for errors
        unimplemented!()
    }
}

pub unsafe extern "C" fn interrupt_handler(
    excp: *mut nk_bindings::excp_entry_t,
    vec: nk_bindings::excp_vec_t,
    state: *mut c_void,
) -> c_int {
    unimplemented!()
}
