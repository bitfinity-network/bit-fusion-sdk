#!/bin/bash

set -e

killall -9 node || true
killall -9 icx-proxy || true
dfx stop

start_icx() {
    killall icx-proxy
    sleep 2
    # Start ICX Proxy
    dfx_local_port=$(dfx info replica-port)
    icx-proxy --fetch-root-key --address 127.0.0.1:8545 --dns-alias 127.0.0.1:bd3sg-teaaa-aaaaa-qaaba-cai --replica http://localhost:$dfx_local_port &
    sleep 2
}

cargo build --tests -p integration-tests --features dfx_tests

rm -f dfx_tests.log

set +e
dfx start --background --clean 2> dfx_tests.log
start_icx

local-ssl-proxy --source 8001 --target 8545 --key ./btc-deploy/mkcert/localhost+3-key.pem --cert ./btc-deploy/mkcert/localhost+3.pem &

dfx identity use max
wallet_principal=$(dfx identity get-wallet)
echo "Wallet Principal: $wallet_principal"
dfx ledger fabricate-cycles --t 1000000 --canister $wallet_principal

sleep 10

cargo test -p integration-tests --features dfx_tests $1

killall -9 node || true
killall -9 icx-proxy || true

dfx stop
