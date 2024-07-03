#!/bin/bash

export ORD_BITCOIN_RPC_USERNAME=ic-btc-integration
export ORD_BITCOIN_RPC_PASSWORD="QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E="
LOGFILE=./target/dfx_tests.log

usage() {
  echo "Usage: $0 [options]"
  echo "Options:"
  echo "  -h, --help                                      Display this help message"
  echo "  --docker                                        Setup docker containers"
  echo "  --github-ci                                     Use this flag when running in GitHub CI"
}

setup_docker() {
    set -e
    PREV_PATH=$(pwd)
    cd btc-deploy/
    docker compose up -d --build || docker-compose up -d --build
    cd $PREV_PATH
    set +e
}

stop_docker() {
    PREV_PATH=$(pwd)
    cd btc-deploy/
    docker compose down || docker-compose down
    cd $PREV_PATH
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

# set dfxvm to use the correct version
if [ "$GITHUB_CI" -gt 0 ]; then
    dfxvm default 0.18.0
fi

killall -9 icx-proxy || true
dfx stop

if [ "$DOCKER" -gt 0 ]; then
    setup_docker
fi

start_icx() {
    killall icx-proxy
    sleep 2
    # Start ICX Proxy
    dfx_local_port=$(dfx info replica-port)
    icx-proxy --fetch-root-key --address 0.0.0.0:8545 --dns-alias 0.0.0.0:bd3sg-teaaa-aaaaa-qaaba-cai --replica http://localhost:$dfx_local_port &
    sleep 2
}

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

BTC_ENDPOINT_READY=255
BTC_ENDPOINT_ELAPSED=0
BTC_ENDPOINT_SLEEP_SECONDS=5
BTC_ENDPOINT_TIMEOUT_SECONDS=60

while [ "$BTC_ENDPOINT_READY" -ne 0 ]; do
  curl --verbose --user ${ORD_BITCOIN_RPC_USERNAME}:${ORD_BITCOIN_RPC_PASSWORD} --data-binary '{"jsonrpc":"1.0","id":"curltest","method":"getblockchaininfo","params":[]}' -H 'content-type:text/plain;' http://127.0.0.1:18443/
  BTC_ENDPOINT_READY=$?
  echo "BITCOIN READY $BTC_ENDPOINT_READY"
  sleep $BTC_ENDPOINT_SLEEP_SECONDS
  let BTC_ENDPOINT_ELAPSED=$BTC_ENDPOINT_ELAPSED+$BTC_ENDPOINT_SLEEP_SECONDS
  if [ "$BTC_ENDPOINT_ELAPSED" -gt $BTC_ENDPOINT_TIMEOUT_SECONDS ]; then
    echo "Bitcoin endpoint not ready after $BTC_ENDPOINT_TIMEOUT_SECONDS seconds; aborting"
    exit 1
  fi
done

cargo test -p integration-tests --features dfx_tests $@
TEST_RESULT=$?

killall -9 icx-proxy || true

dfx stop

if [ "$DOCKER" -gt 0 ]; then
    stop_docker
fi

exit $TEST_RESULT
