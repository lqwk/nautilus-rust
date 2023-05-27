use crate::kernel::bindings;
use core::cell::UnsafeCell;
use lock_api::{GuardSend, RawMutex};

// These functions are wrappers around C macros. See `glue.c`.
extern "C" {
    fn spin_lock_irq(lock: *mut bindings::spinlock_t) -> u8;
    fn spin_unlock_irq(lock: *mut bindings::spinlock_t, flags: u8);
}

pub type IRQLock<T> = lock_api::Mutex<_NkIrqLock, T>;
//pub type IRQLockGuard<'a, T> = lock_api::MutexGuard<'a, NkIrqLock, T>;

#[doc(hidden)]
pub struct _NkIrqLock {
    spinlock: UnsafeCell<bindings::spinlock_t>,
    state_flags: UnsafeCell<u8>,
}

impl _NkIrqLock {
    // `spinlock_init` simply sets the given `u32` to 0
    // `state_flags` can have an arbitrary initial value
    const fn new() -> Self {
        _NkIrqLock {
            spinlock: UnsafeCell::new(0),
            state_flags: UnsafeCell::new(0),
        }
    }
}

// SAFETY: Nautilus' spinlock can be sent between threads.
unsafe impl Send for _NkIrqLock {}
// SAFETY: Nautilus' spinlock can be accessed concurrently.
unsafe impl Sync for _NkIrqLock {}

// SAFETY: `unlock` must only be called when the lock is held.
// Do not use `_NkIrqLock` directly, instead use `IRQLock`,
// for which `lock_api` makes everything footgun-proof.
unsafe impl RawMutex for _NkIrqLock {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: _NkIrqLock = _NkIrqLock::new();

    type GuardMarker = GuardSend;

    fn lock(&self) {
        let lock_ptr = self.spinlock.get();

        // SAFETY: Both the pointer to `state_flags` and `lock_ptr`
        // are valid pointers, as they just came from UnsafeCell::get.
        // Thread-safety is guaranteed by the lock itself.
        unsafe { *self.state_flags.get() = spin_lock_irq(lock_ptr); }
    }

    fn try_lock(&self) -> bool {
        unimplemented!()
    }

    unsafe fn unlock(&self) {
        let lock_ptr = self.spinlock.get();

        // SAFETY: Both the pointer to `state_flags` and `lock_ptr`
        // are valid pointers, as they just came from UnsafeCell::get.
        // Thread-safety is guaranteed by the lock itself.
        unsafe { spin_unlock_irq(lock_ptr, *self.state_flags.get()); }
    }
}
