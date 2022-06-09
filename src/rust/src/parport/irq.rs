use core::{
    ffi::{c_int, c_void},
    fmt::Error,
};

use alloc::sync::Arc;

use crate::nk_bindings;

use super::{lock::IRQLock, Parport};

pub struct Irq {
    num: u16,
}

impl Irq {
    pub fn new(num: u16) -> Self {
        Irq { num }
    }

    pub unsafe fn register(&mut self, parport: Arc<IRQLock<Parport>>) -> Result<(), Error> {
        let handler = interrupt_handler;
        let parport_ptr = Arc::into_raw(parport);
        let result;
        unsafe {
            result = nk_bindings::register_irq_handler(
                self.num,
                Some(handler),
                parport_ptr as *mut c_void,
            );
        }
        match result {
            0 => Ok(()),
            _ => Err(Error),
        }
    }
}

unsafe fn deref_locked_state(state: *mut c_void) -> Arc<IRQLock<Parport>> {
    // caller must guarantee `state`, and the object it points to, was not mutated
    unsafe { Arc::from_raw(state as *const IRQLock<Parport>) }
}

pub unsafe extern "C" fn interrupt_handler(
    _excp: *mut nk_bindings::excp_entry_t,
    _vec: nk_bindings::excp_vec_t,
    state: *mut c_void,
) -> c_int {
    let p = unsafe { deref_locked_state(state) };
    let mut l = p.lock();
    l.set_ready();

    // IRQ_HANDLER_END
    unsafe {
        nk_bindings::apic_do_eoi();
    }
    0
    // l falls out of scope here, releasing the lock and reenabling interrupts after
    // IRQ_HANDLER_END. Redundant, but should work correctly.
}
