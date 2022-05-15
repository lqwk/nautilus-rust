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
src/rust/nk_rust.o:
	(cd src/rust && cargo -Zbuild-std rustc --target x86_64-nautilus-core-kernel.json --release -- --emit=obj)
	(cd src/rust && cp target/x86_64-nautilus-core-kernel/release/deps/nk_rust*.o nk_rust.o)
	(cd src/rust && cp target/x86_64-nautilus-core-kernel/release/libnk_rust.a .)

clean:
	(cd src/rust && cargo clean)
	(cd src/rust && rm nk_rust.o libnk_rust.a)
