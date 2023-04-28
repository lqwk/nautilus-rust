use bindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=bindgen_wrapper.h");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate bindings for.
        .header("bindgen_wrapper.h")
        // set the root directory for nested `#include`s
        .clang_arg("-F../../include/")
        // prefix types with `core::ffi` for a `no_std` environment
        .ctypes_prefix("core::ffi")
        // `core` instead of `libstd`
        .use_core()
        .rust_target(bindgen::RustTarget::Nightly)
        // use with caution - NK's C code is built with GCC
        // whereas bindgen (and rustc) use clang.
        //.emit_builtins()
        //
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings (did you run gen_wrapper.sh?)");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
