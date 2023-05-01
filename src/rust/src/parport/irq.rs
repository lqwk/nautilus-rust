use core::{
    ffi::{c_int, c_void},
    ptr::null,
};

use alloc::sync::Arc;

use crate::kernel::bindings;
use crate::prelude::*;

use super::{lock::IRQLock, interrupt_handler};

pub struct Irq<T> {
    num: u8,
    registered: bool,
    arc_ptr: *const IRQLock<T>,
}

// Added the missing type parameter <T> in the impl block
impl<T> Irq<T> {
    pub fn new(num: u8) -> Self {
        Irq {
            num,
            registered: false,
            arc_ptr: null(),
        }
    }

    pub unsafe fn register(&mut self, parport: Arc<IRQLock<T>>) -> Result { 
        if self.registered {
            return Err(-1);
        }

        let handler = interrupt_handler::<T>; // Added the type parameter <T> for the handler
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

impl<T> Drop for Irq<T> { // Added the missing type parameter <T>
    fn drop(&mut self) {
        if self.registered {
            unsafe {
                bindings::nk_mask_irq(self.num);
                Arc::from_raw(self.arc_ptr);
            }
        }
    }
}

pub unsafe fn deref_locked_state<'a, T>(state: *mut c_void) -> &'a IRQLock<T> {
    // caller must guarantee `state`, and the object it points to, was not mutated
    //
    // caller must not drop the strong reference count of the containing `Arc` to 0 while
    // the returned reference exists
    let l = state as *const IRQLock<T>;
    unsafe { l.as_ref() }.unwrap()
}

