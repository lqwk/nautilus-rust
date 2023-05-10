use core::{
    ffi::{c_int, c_void},
    ptr::null,
};

use alloc::sync::Arc;

use crate::kernel::bindings;
use crate::prelude::*;

use super::{lock::IRQLock};

pub struct Irq<T> {
    num: u8,
    registered: bool,
    arc_ptr: *const IRQLock<T>,
}

impl<T> Irq<T> {
    pub fn new(num: u8) -> Self {
        Irq {
            num,
            registered: false,
            arc_ptr: null(),
        }
    }

    pub unsafe fn register(&mut self, 
        parport: Arc<IRQLock<T>>, 
        handler: unsafe extern "C" fn(
            *mut bindings::excp_entry_t,
            bindings::excp_vec_t, 
            *mut c_void) -> c_int) 
    -> Result {
        if self.registered {
            return Err(-1);
        }

        //Slet handler = interrupt_handler;
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

impl<T> Drop for Irq<T> {
    fn drop(&mut self) {
        if self.registered {
            unsafe {
                bindings::nk_mask_irq(self.num);
                Arc::from_raw(self.arc_ptr);
            }
        }
    }
}