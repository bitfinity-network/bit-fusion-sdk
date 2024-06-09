#!/usr/bin/env sh
# Getting external dependencies for this project

script_dir=$(dirname $0)
wasm_dir_default=$(realpath "${script_dir}/../.artifact")

WASMS_DIR=${WASMS_DIR:-$wasm_dir_default}

EVMC_TAG=${EVMC_TAG:-"v0.2.0-2207-g78d7fc4a-377-g5b755d89-27-g9646de62-1-g5d5fb9b1"}
EVMC_EVM_TGZ=${EVMC_EVM_TGZ:-"evm-testnet-v0.2.0-2207-g78d7fc4a-377-g5b755d89-27-g9646de62-1-g5d5fb9b1.tar.gz"}
EVMC_SIG_TGZ=${EVMC_SIG_TGZ:-"signature-verification-v0.2.0-2207-g78d7fc4a-377-g5b755d89-27-g9646de62-1-g5d5fb9b1.tar.gz"}

echo "Downloading evm-canister release \"$EVMC_TAG\"'"

cd "$script_dir"
./gh_get_priv_release.sh "$WASMS_DIR" bitfinity-network evm-canister "$EVMC_TAG" "$EVMC_EVM_TGZ" "$EVMC_SIG_TGZ"
