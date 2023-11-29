#!/usr/bin/env sh
set -e
set -x

# Add unit tests for all the packages
cargo test

# Units tests with the testnet feature
cargo test --features "testnet"

# Test evm_core backends (default set of features is empty).
# This includes the execution of the Ethereum client tests from https://github.com/ethereum/tests
#
# For Ethereum transactions that reaches the max call depth (1024) revm can use more stack
# space than what is allocated by default.
# See https://github.com/bluealloy/revm/issues/305
ENABLE_ETHEREUM_TESTS=true RUST_MIN_STACK=67108864 cargo test -p evm_core --features "ethereum_test"

# WASM integration tests
cargo test -p integration-tests --no-default-features --features "pocket_ic_integration_test"
