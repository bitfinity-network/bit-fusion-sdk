#!/usr/bin/env sh
set -e
set -x

# Add unit tests for all the packages
cargo test

# WASM integration tests
cargo test -p integration-tests --no-default-features --features "pocket_ic_integration_test"
