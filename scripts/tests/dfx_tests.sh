#!/bin/bash

export ORD_BITCOIN_RPC_USERNAME=ic-btc-integration
export ORD_BITCOIN_RPC_PASSWORD="QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E="
LOGFILE=./target/dfx_tests.log

setup_docker() {
    PREV_PATH=$(pwd)
    cd btc-deploy/
    docker-compose up -d --build
    cd $PREV_PATH
}

stop_docker() {
    PREV_PATH=$(pwd)
    cd btc-deploy/
    docker-compose down
    cd $PREV_PATH
}

kill_ssl_proxy() {
    PID="$(ps aux | grep local-ssl-proxy | grep -v grep | awk '{print $2}')"
    if [ -n "$PID" ]; then
        kill -9 $PID
    fi
}

WITH_DOCKER="0"
if [ "$1" == "--docker" ]; then
    WITH_DOCKER="1"
    shift
fi

# set dfxvm to use the correct version and use docker
if [ "$1" == "--github-ci" ]; then
    shift
    WITH_DOCKER="1"
    dfxvm default 0.16.1
fi


kill_ssl_proxy || true
killall -9 icx-proxy || true
dfx stop

if [ "$WITH_DOCKER" -eq 1 ]; then
    setup_docker
fi

set -e

start_icx() {
    killall icx-proxy
    sleep 2
    # Start ICX Proxy
    dfx_local_port=$(dfx info replica-port)
    icx-proxy --fetch-root-key --address 127.0.0.1:8545 --dns-alias 127.0.0.1:bd3sg-teaaa-aaaaa-qaaba-cai --replica http://localhost:$dfx_local_port &
    sleep 2
}

rm -f "$LOGFILE"

set +e
dfx start --background --clean --enable-bitcoin 2> "$LOGFILE"
start_icx

local-ssl-proxy --source 8002 --target 8545 --key ./btc-deploy/mkcert/localhost+3-key.pem --cert ./btc-deploy/mkcert/localhost+3.pem &

dfx identity use max
wallet_principal=$(dfx identity get-wallet)
echo "Wallet Principal: $wallet_principal"
dfx ledger fabricate-cycles --t 1000000 --canister $wallet_principal

sleep 10

cargo test -p integration-tests --features dfx_tests $1

kill_ssl_proxy || true
killall -9 icx-proxy || true

dfx stop

if [ "$WITH_DOCKER" -eq 1 ]; then
    stop_docker
fi
