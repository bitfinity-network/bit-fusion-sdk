#!/bin/bash

set -e
set -x

export ORD_BITCOIN_RPC_USERNAME=ic-btc-integration
export ORD_BITCOIN_RPC_PASSWORD="QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E="
LOGFILE=./target/dfx_tests.log
ORD_DATA=$PWD/src/integration-tests/target/ord
ORDW="ord -r --data-dir $ORD_DATA --index-runes wallet --server-url http://localhost:8000"
# Get bitcoin-cli
BITCOIN=$(command -v bitcoin-cli || command -v bitcoin-core.cli)
BITCOIN="$BITCOIN -conf=$PWD/btc-deploy/bitcoin.conf"

usage() {
  echo "Usage: $0 [options]"
  echo "Options:"
  echo "  -h, --help                                      Display this help message"
  echo "  --docker                                        Setup docker containers"
  echo "  --github-ci                                     Use this flag when running in GitHub CI"
}

setup_docker() {
    PREV_PATH=$(pwd)
    rm -rf $ORD_DATA/
    cd btc-deploy/
    rm -rf bitcoin-data/
    mkdir -p bitcoin-data/
    docker compose up -d --build --force-recreate
    cd $PREV_PATH
}

stop_docker() {
    PREV_PATH=$(pwd)
    cd btc-deploy/
    docker compose down
    cd $PREV_PATH
}

start_icx() {
    killall icx-proxy
    sleep 2
    # Start ICX Proxy
    dfx_local_port=$(dfx info replica-port)
    icx-proxy --fetch-root-key --address 0.0.0.0:8545 --dns-alias 0.0.0.0:bd3sg-teaaa-aaaaa-qaaba-cai --replica http://localhost:$dfx_local_port &
    sleep 2
}

get_wallet_address() {
    WALLET_NAME="$1"

    local i=0
    local found=0
    while true; do
        local wallet_item=$($BITCOIN listwallets | jq -r .[$i])
        if [ $wallet_item = "null" ]; then
            break
        elif [ $wallet_item = $WALLET_NAME ]; then
            found=1
            break
        else
            let i=i+1
        fi
    done

    if [ $found -eq 0 ]; then
        echo ""
    else 
        WALLET_ADDRESS=$($BITCOIN -rpcwallet=$WALLET_NAME getnewaddress)
        echo "$WALLET_ADDRESS"
    fi
}

create_or_reuse_wallet() {
    WALLET_NAME="$1"
    
    WALLET_ADDRESS=$(get_wallet_address $WALLET_NAME)

    if [ -z "$WALLET_ADDRESS" ]; then
        $BITCOIN createwallet $WALLET_NAME > /dev/null || true

        WALLET_ADDRESS=$($BITCOIN -rpcwallet=$WALLET_NAME getnewaddress)
        if [ -z "$WALLET_ADDRESS" ]; then
            echo "Failed to create wallet $WALLET_NAME"
            exit 1
        fi
    fi
    
    echo "$WALLET_ADDRESS"
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

# setup wallet
WALLET_TEST="admin"
export WALLET_TEST_ADDRESS=$(create_or_reuse_wallet $WALLET_TEST)
echo "$WALLET_TEST address: $WALLET_TEST_ADDRESS"

# Ensure we have some BTC to spend
$BITCOIN generatetoaddress 101 $WALLET_TEST_ADDRESS
echo "Generated 101 blocks for $WALLET_TEST"

# create inscriptions
WALLET_ORD="ord"
$ORDW create
sleep 10

ORD_ADDRESS=$($ORDW receive | jq -r .addresses[0])
if [ -z "$ORD_ADDRESS" ]; then
    echo "Failed to get ord address"
    exit 1
fi
echo "ORD address: $ORD_ADDRESS"

$BITCOIN -rpcwallet=$WALLET_TEST sendtoaddress $ORD_ADDRESS 10 &> /dev/null
$BITCOIN -rpcwallet=$WALLET_TEST generatetoaddress 1 $WALLET_TEST_ADDRESS &> /dev/null

echo "Sent 10 BTC to ord wallet address $ORD_ADDRESS"
sleep 5

$ORDW batch --fee-rate 10 --batch ./scripts/tests/runes/rune.yaml &
sleep 1
$BITCOIN -rpcwallet=$WALLET_TEST generatetoaddress 10 $WALLET_TEST_ADDRESS

sleep 5
$BITCOIN -rpcwallet=$WALLET_TEST generatetoaddress 1 $WALLET_TEST_ADDRESS
sleep 30

ord -r --data-dir $ORD_DATA --index-runes runes

# run tests
set +e
cargo test -p integration-tests --features dfx_tests runes_bridging_flow
TEST_RESULT=$?
cat "$LOGFILE" | grep "ERROR"
set -e

killall -9 icx-proxy || true

dfx stop

if [ "$DOCKER" -gt 0 ]; then
    stop_docker
fi

exit $TEST_RESULT
