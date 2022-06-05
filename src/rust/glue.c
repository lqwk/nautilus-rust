#include <nautilus/nautilus.h>
#include <nautilus/shell.h>
#include <nautilus/spinlock.h>
#include <nautilus/cpu.h>

// Rust function we will call from C
extern int example_shell_entry(char *, void *);

static struct shell_cmd_impl rust_example_impl = {
    .cmd = "rust",
    .help_str = "rust",
    .handler = example_shell_entry,
};

// note that this is currently a macro, and cannot
// be called using the Rust FFI
nk_register_shell_cmd(rust_example_impl);

// parport

// direct wrappers around inline functions
static uint8_t spin_lock_irq(spinlock_t *lock) {
  return spin_lock_irq_save(lock);
}
static void spin_unlock_irq(spinlock_t *lock, uint8_t flags) {
  spin_unlock_irq_restore(lock, flags);
}

extern int parport_shell_entry(char *, void *);
static struct shell_cmd_impl rust_parport_impl = {
    .cmd = "parport",
    .help_str = "parport",
    .handler = parport_shell_entry,
};
nk_register_shell_cmd(rust_parport_impl);
