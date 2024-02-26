#!/usr/bin/env sh

set -e
set -x

# Use IC release of 2024-02-21: https://github.com/dfinity/ic/releases/tag/release-2024-02-21_23-01-p2p
export IC_VERSION=85bd56a70e55b2cea75cae6405ae11243e5fdad8

mkdir -p .artifact

curl --fail -o .artifact/ic-ckbtc-minter.wasm.gz "https://download.dfinity.systems/ic/$IC_VERSION/canisters/ic-ckbtc-minter.wasm.gz"
curl --fail -o .artifact/ic-icrc1-ledger.wasm.gz "https://download.dfinity.systems/ic/$IC_VERSION/canisters/ic-icrc1-ledger.wasm.gz"
curl --fail -o .artifact/ic-btc-canister.wasm.gz "https://download.dfinity.systems/ic/$IC_VERSION/canisters/ic-btc-canister.wasm.gz"
curl --fail -o .artifact/ic-ckbtc-kyt.wasm.gz "https://download.dfinity.systems/ic/$IC_VERSION/canisters/ic-ckbtc-kyt.wasm.gz"
