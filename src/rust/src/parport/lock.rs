use crate::kernel::bindings;
use core::cell::UnsafeCell;
use lock_api::{GuardSend, RawMutex};

extern "C" {
    fn spin_lock_irq(lock: *mut bindings::spinlock_t) -> u8;
    fn spin_unlock_irq(lock: *mut bindings::spinlock_t, flags: u8);
}

pub type IRQLock<T> = lock_api::Mutex<NkIrqLock, T>;
//pub type IRQLockGuard<'a, T> = lock_api::MutexGuard<'a, NkIrqLock, T>;

pub struct NkIrqLock {
    spinlock: UnsafeCell<bindings::spinlock_t>,
    state_flags: UnsafeCell<u8>,
}

impl NkIrqLock {
    // `spinlock_init()` simply sets the given `u32` to 0
    // `state_flags` can have an arbitrary initial value
    const fn new() -> Self {
        NkIrqLock {
            spinlock: UnsafeCell::new(0),
            state_flags: UnsafeCell::new(0),
        }
    }
}

// SAFETY: Nautilus' spinlock is thread-safe.
unsafe impl Send for NkIrqLock {}
unsafe impl Sync for NkIrqLock {}

unsafe impl RawMutex for NkIrqLock {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: NkIrqLock = NkIrqLock::new();

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
