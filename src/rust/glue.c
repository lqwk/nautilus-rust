#include <nautilus/nautilus.h>
#include <nautilus/cpu_state.h>
#include <nautilus/vc.h>
#include <nautilus/spinlock.h>

// parport

// direct wrappers around inline functions and macros

void debug_error_print(char* s) {
    if (__cpu_state_get_cpu()) {
        int _p=preempt_is_disabled();
        preempt_disable();
        struct nk_thread *_t = get_cur_thread();
        nk_vc_log_wrap(s,
                my_cpu_id(),
                in_interrupt_context() ? "I" :"",
                _p ? "" : "P",
                _t ? _t->tid : 0,
                _t ? _t->is_idle ? "*idle*" : _t->name[0]==0 ? "*unnamed*" : _t->name : "*none*");
        preempt_enable();
    } else {
        int _p=preempt_is_disabled();
        preempt_disable();
        nk_vc_log_wrap(s,
                in_interrupt_context() ? "I" :"",
                _p ? "" : "P");
        preempt_enable();
    }
 }

uint8_t spin_lock_irq(spinlock_t *lock) {
    return spin_lock_irq_save(lock);
}

void spin_unlock_irq(spinlock_t *lock, uint8_t flags) {
  spin_unlock_irq_restore(lock, flags);
}
