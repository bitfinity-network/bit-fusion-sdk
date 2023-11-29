#!/usr/bin/env sh
set -e
set -x

# Stop the dfx and icx-proxy
stop_dfx() {
    dfx stop || true
    killall dfx || true
    killall dfx -s SIGKILL || true
    killall icx-proxy || true
    killall icx-proxy -s SIGKILL || true
}

# Execute wasm integration tests.

./scripts/dfx/tests/start_dfx.sh

cargo test -p integration-tests --no-default-features --features "dfx_integration_test"

stop_dfx

sleep 5

stop_dfx

exit 0
