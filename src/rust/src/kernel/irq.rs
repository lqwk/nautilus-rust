use core::ffi::{c_int, c_void};
use core::marker::PhantomData;
use alloc::sync::Arc;

use crate::kernel::{
    error::{Result, ResultExt},
    print::make_logging_macros,
    bindings
};

make_logging_macros!("irq");

/// Manages resources associated with an IRQ handler.
/// 
/// # Invariants
///
/// `data` is a valid, non-null pointer.
#[derive(Debug)]
struct InternalRegistration<T> {
    int_vec: u16,
    data: *mut c_void,
    _p: PhantomData<Arc<T>>,
}

// SAFETY: `data` is a raw pointer with no thread affinity. The C
// interrupt handler using `data` does not modify `data` or move
// it's referrent. We only store `data` in an `_InternalRegistration`
// so that we can later reclaim the memory it points to. So it is
// safe to send an `_InternalRegistration` between threads. `Send`
// is important to implement here so that, if some type `T` contains
// an `irq::Registration`, then `Mutex<NkIrqLock, T>` (from `lock_api`)
// implements `Sync` and `Send`.
//
// Rust-for-Linux's analogous `InternalRegistration` is not `Send`
// (which confuses me because their spinlocks can't be `Send` and
// `Sync` if they guard something containing an IRQ registration as
// mentioned above). In short, I am not 100% confident that this
// `impl` is truly safe, and if you are noticing odd behavior with
// interrupts (e.g. data races), then you may want to consider the
// implications of this line.
unsafe impl<T> Send for InternalRegistration<T> {}

impl<T> InternalRegistration<T> {
    /// Registers a new irq handler.
    unsafe fn try_new(
        irq: u16,
        handler: Option<unsafe extern "C" fn(
            *mut bindings::excp_entry_t,
            bindings::excp_vec_t,
            *mut c_void,
        ) -> c_int>,
        data: Arc<T>,
    ) -> Result<Self> {
        let ptr = Arc::into_raw(data) as *mut c_void;

        let result = Result::from_error_code(
            // SAFETY: `ptr` remains valid as long as the registration is alive.
            unsafe { bindings::register_irq_handler(irq, handler, ptr) }
        );

        match result {
            Ok(_) => {
                // SAFETY: FFI call.
                unsafe { bindings::nk_unmask_irq(irq as u8); }

                debug!("Successfully registered IRQ {irq}.");
                Ok(Self {
                    int_vec: irq,
                    data: ptr,
                    _p: PhantomData,
                })
            },
            Err(e) => {
                error!("Unable to register IRQ {irq}. Error code {e}.");
                // SAFETY: `ptr` came from a previous call to `into_raw`.
                unsafe { let _ = Arc::from_raw(ptr as *mut T); }
                Err(e)
            },
        }
    }
}

impl<T> Drop for InternalRegistration<T> {
    fn drop(&mut self) {
        debug!("Dropping a registration for IRQ {}.", self.int_vec);

        // SAFETY: When `try_new` succeeds, the irq was successfully unmasked,
        // so it is ok to mask it here.
        unsafe { bindings::nk_mask_irq(self.int_vec as u8); }

        // SAFETY: This matches the call to `into_raw` from `try_new` in the success case.
        unsafe { Arc::from_raw(self.data as *mut T); }
    }
}

/// An irq handler.
pub trait Handler {
    /// The context data associated with and made available to the handler.
    type State: Send + Sync;

    /// Called from interrupt context when the irq happens.
    fn handle_irq(data: &Self::State) -> Result;
}

/// The registration of an interrupt handler.
#[derive(Debug)]
pub struct Registration<H: Handler>(InternalRegistration<H::State>);

impl<H: Handler> Registration<H> {
    /// Registers a new irq handler.
    pub fn try_new(irq: u16, data: Arc<H::State>) -> Result<Self> {
        // SAFETY: `handler` only calls `Arc::clone` on `raw_state`.
        Ok(Self(unsafe {
            InternalRegistration::try_new(irq, Some(Self::handler), data)?
        }))
    }

    unsafe extern "C" fn handler(
        _excp: *mut bindings::excp_entry_t,
        _vec: bindings::excp_vec_t,
        raw_state: *mut core::ffi::c_void,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the irq is unregistered.
        let state = unsafe { (raw_state as *const H::State).as_ref() }.unwrap();
        let ret = H::handle_irq(state).as_error_code();

        // SAFETY: `handler` runs in an interrupt context. `H::handle_irq` has terminated
        // at this point, so it is safe to signal an end-of-interrupt.
        unsafe { bindings::apic_do_eoi(); };

        ret
    }
}
