#include <nautilus/nautilus.h>
#include <nautilus/shell.h>

// Rust function we will call from C
extern int example_shell_entry(char *, void *);

static struct shell_cmd_impl rust_impl = {
    .cmd = "rust",
    .help_str = "rust",
    .handler = example_shell_entry,
};

// note that this is currently a macro, and cannot
// be called using the Rust FFI
nk_register_shell_cmd(rust_impl);
