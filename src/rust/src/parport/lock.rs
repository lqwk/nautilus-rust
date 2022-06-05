use lock_api::{GuardSend, RawMutex};

use crate::nk_bindings;

extern "C" {
    fn spin_lock_irq(lock: *mut nk_bindings::spinlock_t) -> u8;
    fn spin_unlock_irq(lock: *mut nk_bindings::spinlock_t, flags: u8);
}

pub type IRQLock<T> = lock_api::Mutex<NkIrqLock, T>;
pub type IRQLockGuard<'a, T> = lock_api::MutexGuard<'a, NkIrqLock, T>;

// deriving default because `spinlock_init()` simply sets the
// given `u32` to 0, and `state_flags` can have an arbitrary initial value
#[derive(Debug, Default)]
struct NkIrqLock {
    spinlock: nk_bindings::spinlock_t,
    state_flags: u8,
}

unsafe impl RawMutex for NkIrqLock {
    const INIT: NkIrqLock = NkIrqLock::default();
    type GuardMarker = GuardSend;

    fn lock(&self) {
        let lock_ptr = &mut self.spinlock as *mut u32;
        self.state_flags = unsafe { spin_lock_irq(lock_ptr) };
    }

    fn try_lock(&self) -> bool {
        unimplemented!()
    }

    unsafe fn unlock(&self) {
        let lock_ptr = &mut self.spinlock as *mut u32;
        unsafe { spin_unlock_irq(lock_ptr, self.state_flags) };
    }
}
