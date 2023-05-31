# install rustup
if [ -z "$(which rustup)" ]
then
    curl https://sh.rustup.rs -sSf | sh -s -- -y
fi

# setup path
# source ~/.cargo/env

# install nightly toolchain and compnent rust-rc
rustup toolchain install nightly --component rust-src

# install bindgen dependencies
apt install llvm-dev libclang-dev clang

# also need this for bindgen to understand <stddint.h>
sudo apt-get install gcc-multilib

# install bindgen for generating rust bindings
cargo install bindgen
