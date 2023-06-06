#include <nautilus/nautilus.h>
#include <nautilus/cpu_state.h>
#include <nautilus/vc.h>
#include <nautilus/spinlock.h>
#include <nautilus/thread.h>
#include <dev/virtio_pci.h>
#include <dev/vga.h>

// direct wrappers around inline functions and macros

void _glue_log_print(char* s) {
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

nk_thread_t* _glue_get_cur_thread() {
    return get_cur_thread();
}

void _glue_spin_lock(spinlock_t *lock) {
    spin_lock(lock);
}

void _glue_spin_unlock(spinlock_t *lock) {
    spin_unlock(lock);
}

uint8_t _glue_spin_lock_irq(spinlock_t *lock) {
    return spin_lock_irq_save(lock);
}

void _glue_spin_unlock_irq(spinlock_t *lock, uint8_t flags) {
    spin_unlock_irq_restore(lock, flags);
}

uint8_t _glue_irq_save(void) {
    return irq_disable_save();
}

void _glue_irq_restore(uint8_t iflag) {
    irq_enable_restore(iflag);
}

void _glue_yield() {
    nk_yield();
}

void _glue_mbarrier() {
    mbarrier();
}

void _glue_virtio_pci_atomic_store_u16(uint16_t* destptr, uint16_t value) {
    virtio_pci_atomic_store(destptr, value);
}

uint16_t _glue_virtio_pci_atomic_load_u16(uint16_t* srcptr) {
    return virtio_pci_atomic_load(srcptr);
}

void _glue_vga_copy_out(void* dest, uint32_t n) {
    vga_copy_out(dest, n);
}

void _glue_vga_copy_in(void* src, uint32_t n) {
    vga_copy_in(src, n);
}
