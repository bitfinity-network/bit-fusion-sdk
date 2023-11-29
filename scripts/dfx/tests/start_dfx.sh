#!/usr/bin/env sh
# This script is used to start the integration environment.

set -e
# Trying to set wasms dir automatically for non CI execution
if [ "$CI" != "true" ]; then
    if [ -z "$WASMS_DIR" ] && [ -d ".artifact" ]; then
        export WASMS_DIR="$(pwd)/.artifact"
        echo "WASMS_DIR was defined with path: $WASMS_DIR"
    fi
fi

if [ -z "$WASMS_DIR" ] && [ -z "$DFX_WASMS_DIR" ]; then
    echo "Neither WASMS_DIR nor DFX_WASMS_DIR env variables were defined!"
    echo "Test would likely to fail."
    exit 42
fi

echo "Reading identity from: $HOME/.config/dfx/identity/"

dfx stop
dfx start --background --clean --host 127.0.0.1:8000 --artificial-delay 0

sleep 2

set +e
dfx identity new --storage-mode=plaintext alex
dfx identity new --storage-mode=plaintext max
dfx identity new --storage-mode=plaintext alice
set -e

dfx identity use max
wallet_principal=$(dfx identity get-wallet)

echo "Max principal: '$wallet_principal'"
dfx ledger fabricate-cycles --t 100000 --canister $wallet_principal

dfx identity use alex
