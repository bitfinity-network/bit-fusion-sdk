#!/bin/bash

set -e
set -x

LOGFILE=./target/dfx_tests.log

usage() {
    echo "Usage: $0 [options]"
    echo "Options:"
    echo "  -h, --help                                      Display this help message"
    echo "  --docker                                        Setup docker containers"
    echo "  --github-ci                                     Use this flag when running in GitHub CI"
}

setup_docker() {
    PREV_PATH=$(pwd)
    cd btc-deploy/
    rm -rf bitcoin-data/*
    mkdir -p bitcoin-data/
    rm -rf db-data/*
    mkdir -p db-data/
    docker compose down && docker compose up -d --build
    cd $PREV_PATH
}

stop_docker() {
    PREV_PATH=$(pwd)
    cd btc-deploy/
    docker compose down
    cd $PREV_PATH
}

start_icx() {
    killall icx-proxy || true
    sleep 2
    # Start ICX Proxy
    dfx_local_port=$(dfx info replica-port)
    icx-proxy --fetch-root-key --address 0.0.0.0:8545 --dns-alias 0.0.0.0:bd3sg-teaaa-aaaaa-qaaba-cai --replica http://localhost:$dfx_local_port &
    sleep 2
}

DOCKER="0"
GITHUB_CI="0"

ARGS=$(getopt -o h --long docker,github-ci,help -- "$@")
while true; do
    case "$1" in
    --docker)
        DOCKER="1"
        shift
        ;;

    --github-ci)
        GITHUB_CI="1"
        shift
        ;;

    -h | --help)
        usage
        exit 255
        ;;

  --)
        shift
        break
        ;;

  *)
        break
        ;;
    esac
done

# check bad dfx version
DFX_VERSION=$(dfx --version | awk '{print $2}')
DFX_VERSION_MINOR=$(echo $DFX_VERSION | cut -d. -f2)
if [ "$DFX_VERSION_MINOR" -lt 20 ]; then
    echo "dfx version 0.18.0 doesn't work with bitcoin integration."
    echo "dfx version 0.19.0 wont't build."
    echo "Please upgrade dfx to >=0.20.1"

    exit 1
fi

killall -9 icx-proxy || true
dfx stop

if [ "$DOCKER" -gt 0 ]; then
    setup_docker
fi

rm -f "$LOGFILE"

dfx start --background --clean --enable-bitcoin 2> "$LOGFILE"
start_icx

dfx identity new --storage-mode=plaintext --force max
dfx identity new --storage-mode=plaintext --force alice
dfx identity new --storage-mode=plaintext --force alex
dfx identity use max
wallet_principal=$(dfx identity get-wallet)
echo "Wallet Principal: $wallet_principal"
dfx ledger fabricate-cycles --t 1000000 --canister $wallet_principal

sleep 10

# run tests
cargo test -p integration-tests --features dfx_tests $@
TEST_RESULT=$?

killall -9 icx-proxy || true

dfx stop

if [ "$DOCKER" -gt 0 ]; then
    stop_docker
fi

exit $TEST_RESULT
