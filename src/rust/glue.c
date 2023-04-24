#include <nautilus/spinlock.h>

// parport

// direct wrappers around inline functions
uint8_t spin_lock_irq(spinlock_t *lock) { return spin_lock_irq_save(lock); }
void spin_unlock_irq(spinlock_t *lock, uint8_t flags) {
  spin_unlock_irq_restore(lock, flags);
}
