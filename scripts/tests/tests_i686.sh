#!/usr/bin/env sh
set -e
set -x

export RUST_BACKTRACE=full

# Unit tests for all the packages
cargo test --target i686-unknown-linux-gnu

