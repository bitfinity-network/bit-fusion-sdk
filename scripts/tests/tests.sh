#!/usr/bin/env sh
set -e
set -x

export RUST_BACKTRACE=full

export PROTOC_INCLUDE=${PWD}/proto

INTEGRATION_TESTS_TO_RUN=$1

# Add unit tests for all the packages
cargo test

# WASM integration tests
cargo test -p integration-tests --no-default-features --features "pocket_ic_integration_test" --features state_machine_tests -- $INTEGRATION_TESTS_TO_RUN
