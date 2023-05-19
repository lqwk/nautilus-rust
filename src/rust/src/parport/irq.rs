use core::{
    ffi::{c_int, c_void},
    ptr::null,
};

use alloc::sync::Arc;

use crate::kernel::bindings;
use crate::prelude::*;

use super::{lock::IRQLock, Parport};

pub struct Irq {
    num: u8,
    registered: bool,
    arc_ptr: *const IRQLock<Parport>,
}

impl Irq {
    pub fn new(num: u8) -> Self {
        Irq {
            num,
            registered: false,
            arc_ptr: null(),
        }
    }

    pub unsafe fn register(&mut self, parport: Arc<IRQLock<Parport>>) -> Result {
        if self.registered {
            return Err(-1);
        }
 
        let handler = interrupt_handler;
        self.arc_ptr = Arc::into_raw(parport);
        let result = unsafe {
            bindings::register_irq_handler(
                self.num.into(),
                Some(handler),
                self.arc_ptr as *mut c_void,
            )
        };

        if result == 0 {
            unsafe {
                bindings::nk_unmask_irq(self.num);
            }
            self.registered = true;
            Ok(())
        } else {
            // taking back `Arc` is safe if handler registration never succeeded
            let _ = unsafe { Arc::from_raw(self.arc_ptr) };
            Err(result)
        }
    }
}

impl Drop for Irq {
    fn drop(&mut self) {
        if self.registered {
            unsafe {
                bindings::nk_mask_irq(self.num);
                Arc::from_raw(self.arc_ptr);
            }
        }
    }
}

unsafe fn deref_locked_state<'a>(state: *mut c_void) -> &'a IRQLock<Parport> {
    // caller must guarantee `state`, and the object it points to, was not mutated
    //
    // caller must not drop the strong reference count of the containing `Arc` to 0 while
    // the returned reference exists
    let l = state as *const IRQLock<Parport>;
    unsafe { l.as_ref() }.unwrap()
}

pub unsafe extern "C" fn interrupt_handler(
    _excp: *mut bindings::excp_entry_t,
    _vec: bindings::excp_vec_t,
    state: *mut c_void,
) -> c_int {
    let p = unsafe { deref_locked_state(state) };
    let mut l = p.lock();
    l.set_ready();

    // IRQ_HANDLER_END
    unsafe {
        bindings::apic_do_eoi();
    }
    0
    // l falls out of scope here, releasing the lock and reenabling interrupts after
    // IRQ_HANDLER_END. Redundant, but should work correctly.
}
