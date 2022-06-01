# set path for qemu
export PATH=$PATH:/home/software/qemu4/bin

# install rustup
curl https://sh.rustup.rs -sSf | sh -s -- -y

# setup path
source ~/.cargo/env

# install nightly toolchain and compnent rust-rc
rustup toolchain install nightly --component rust-src

# install bindgen for generating rust bindings
cargo install bindgen
