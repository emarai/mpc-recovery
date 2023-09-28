#!/bin/bash

# Exit on error and set pipefail
set -e
set -o pipefail

OS=$(uname -s)

if [ "$OS" == "Darwin" ]; then
    # export PATH="/opt/homebrew/opt/llvm/bin:$PATH"
    # export LDFLAGS="-L/opt/homebrew/opt/llvm/lib"
    # export CPPFLAGS="-I/opt/homebrew/opt/llvm/include"
    # export TARGET_CC=$(which clang)

    # # Used to link to system libraries like GMP
    # export RUSTFLAGS="
    #     -L /opt/homebrew/lib
    #     -L /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/lib
    #     -C link-arg=-undefined
    #     -C link-arg=dynamic_lookup
    # "
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc
    export CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc
    export CXX_x86_64_unknown_linux_gnu=x86_64-linux-gnu-g++
    export AR_x86_64_unknown_linux_gnu=x86_64-linux-gnu-ar
fi

rustup target add x86_64-unknown-linux-gnu
cargo build \
    --release \
    --target x86_64-unknown-linux-gnu \
    --package mpc-recovery

docker build \
    --file build/Dockerfile.local \
    --tag near/mpc-recovery \
    ./
