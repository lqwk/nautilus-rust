# name of the generated .o and .a file is controlled by
# the crate name in Cargo.toml
obj-y := glue.o nk_rust.o libnk_rust.a

#
# Force this step to happen all the time.  We need to use
# Cargo to do the Rust build because, of course, you must use their
# build environment...
#
#
.PHONY:  src/rust/nk_rust.o clean

# FIXME: As v0.1.85, "fmod", among other floating point
# symbols, seem to have been added to compiler-builtins
# (see https://github.com/rust-lang/compiler-builtins/issues/509).
#
# For some reason, "fmod" is the only of these symbols that ends up
# exposed by the compiler, even though it is only a wrapper around
# the actual fmod with a mangled name, and is never called anywhere
# else in the object file.
#
# Nautilus also ends up defining fmod, and the linker is unable to
# resolve the multiple definitons. So here we do a fragile check
# that "fmod" is really never called and then strip the object of
# this symbol.
#
# This is really hacky and should be fixed.
src/rust/nk_rust.o:
	(cd src/rust && cargo build --release)
	(cd src/rust && cp target/x86_64-nautilus-core-kernel/release/deps/nk_rust*.o nk_rust.o)
	(cd src/rust && cp target/x86_64-nautilus-core-kernel/release/libnk_rust.a .)
	(cd src/rust && ./fmod_hack.sh)

clean:
	(cd src/rust && cargo clean)
	(cd src/rust && rm nk_rust.o libnk_rust.a)
