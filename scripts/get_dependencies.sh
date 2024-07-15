#!/usr/bin/env sh
# Getting external dependencies for this project
# This script works for local development and from from CI

set -e
set -x

script_dir=$(dirname $0)
wasm_dir_default=$(realpath "${script_dir}/../.artifact")

WASM_DIR=${WASM_DIR:-$wasm_dir_default}

# Update this variables to get new release
EVMC_TAG="v0.2.0-2207-g78d7fc4a-377-g5b755d89-27-g9646de62-1-g5d5fb9b1"
EVMC_EVM_TGZ="evm-testnet-v0.2.0-2207-g78d7fc4a-377-g5b755d89-27-g9646de62-1-g5d5fb9b1.tar.gz"
EVMC_SIG_TGZ="signature-verification-v0.2.0-2207-g78d7fc4a-377-g5b755d89-27-g9646de62-1-g5d5fb9b1.tar.gz"
EVM_RPC_TAG="release-2024-05-23"

echo "Downloading evm-canister release \"$EVMC_TAG\"'"

cd "$script_dir"
./gh_get_priv_release.sh "$WASM_DIR" bitfinity-network evm-canister "$EVMC_TAG" "$EVMC_EVM_TGZ" "$EVMC_SIG_TGZ"

echo "Downloading evm-rpc canister"
wget -O "$WASM_DIR/evm_rpc.wasm.gz" "https://github.com/internet-computer-protocol/evm-rpc-canister/releases/download/$EVM_RPC_TAG/evm_rpc.wasm.gz"
