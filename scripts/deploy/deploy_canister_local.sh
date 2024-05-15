#!/bin/bash

source "$(dirname "$0")/deploy_functions.sh"
source "$(dirname "$0")/deploy_test_functions.sh"

IC_NETWORK="local"
BITCOIN_NETWORK="regtest"
INDEXER_URL="https://127.0.0.1:8001"

function usage() {
  echo "Usage: $0 [options] [canisters]..."
  echo "Canisters: erc20-minter, icrc2-minter, rune-bridge, btc-bridge"
  echo "Options:"
  echo "  -h, --help                                      Display this help message"
  echo "  -b, --bitcoin-network <network>                 Bitcoin network (regtest, testnet, mainnet)"
  echo "  -i, --ic-network <network>                      Internet Computer network (local, ic)"
  echo "  -m, --install-mode <mode>                       Install mode (create, init, reinstall, upgrade)"
  echo "  --indexer-url <url>                             Indexer URL"
}

ARGS=$(getopt -o e:b:i:m:h --long bitcoin-network,ic-network,install-mode,indexer-url,help -- "$@")
while true; do
  case "$1" in

    -b|--bitcoin-network)
      BITCOIN_NETWORK="$2"
      shift 2
      ;;

    -i|--ic-network)
      IC_NETWORK="$2"
      shift 2
      ;;

    -m|--install-mode)
      INSTALL_MODE="$2"
      shift 2
      ;;

    --indexer-url)
      INDEXER_URL="$2"
      shift 2
      ;;

    -h|--help)
      usage
      exit 255
      ;;

    --)
      shift
      break
      ;;

    *)
      break
  esac
done

if [ -z "$INSTALL_MODE" ]; then
  echo "Install mode is required"
  usage
  exit 1
fi

# get positional arguments; skip $0, if empty 'all'
CANISTERS_TO_DEPLOY="${@:1}"
if [ -z "$CANISTERS_TO_DEPLOY" ]; then
  CANISTERS_TO_DEPLOY="icrc2-minter erc20-minter rune-bridge btc-bridge"
fi

start_dfx() {
    echo "Attempting to create Alice's Identity"
    set +e

    if [ "$INSTALL_MODE" = "create" ]; then
        echo "Stopping DFX"
        dfx stop
        echo "Starting DFX"
        dfx start --clean --background --enable-bitcoin --artificial-delay 0 2> dfx_stderr.log
    else
        return
    fi

    # Create identity
    dfx identity new --storage-mode=plaintext alice
    dfx identity use alice
    echo "Alice's Identity Created"
}

start_icx() {
    evm_id="$1"
    killall icx-proxy
    sleep 2
    # Start ICX Proxy
    dfx_local_port=$(dfx info replica-port)
    icx-proxy --fetch-root-key --address 127.0.0.1:8545 --dns-alias 127.0.0.1:$evm_id --replica http://localhost:$dfx_local_port &
    sleep 2

    curl -X POST -H "Content-Type: application/json" --data '{"jsonrpc": "2.0", "method": "eth_chainId", "params": [], "id":1}' 'http://127.0.0.1:8545'
}

assert_isset_param () {
  PARAM=$1
  NAME=$2
  if [ -z "$PARAM" ]; then
    echo "$NAME is required"
    usage
    exit 1
  fi
}

start_dfx
set -e

EVM_PRINCIPAL=$(deploy_evm_testnet)
echo "EVM Principal: $EVM_PRINCIPAL"

LOG_SETTINGS="record { enable_console=true; in_memory_records=opt 10000; log_filter=opt \"error,did=debug,evm_core=debug,evm=debug\"; }"
OWNER=$(dfx identity get-principal)
SIGNING_STRATEGY="variant { Local = record { private_key = blob \"\\01\\23\\45\\67\\89\\01\\23\\45\\67\\01\\01\\23\\45\\67\\89\\01\\23\\45\\67\\01\\01\\23\\45\\67\\89\\01\\23\\45\\67\\01\\67\\01\"; } }"

if [ "$INSTALL_MODE" = "create" ] || [ "$INSTALL_MODE" = "init" ]; then
  INSTALL_MODE="install"
fi

if [ "$INSTALL_MODE" != "install" ] && [ "$INSTALL_MODE" != "upgrade" ] && [ "$INSTALL_MODE" != "reinstall" ]; then
  echo "Usage: $0 <create|init|upgrade|reinstall>"
  exit 1
fi

for canister in $CANISTERS_TO_DEPLOY; do
  case $canister in
    "icrc2-minter")
      deploy_icrc2_minter "$IC_NETWORK" "$INSTALL_MODE" "$EVM_PRINCIPAL" "$OWNER" "$SIGNING_STRATEGY" "$LOG_SETTINGS"
      ;;

    "erc20-minter")
      MINTER_ADDRESS=$(dfx canister call icrc2-minter get_minter_canister_evm_address)
      MINTER_ADDRESS=${MINTER_ADDRESS#*\"}
      MINTER_ADDRESS=${MINTER_ADDRESS%\"*}
      # get base bridge contract
      WALLET=$(get_wallet $EVM_PRINCIPAL)
      BASE_BRIDGE_CONTRACT=$(deploy_bft_bridge $EVM_PRINCIPAL $MINTER_ADDRESS $WALLET)
      # get wrapped bridge contract
      WRAPPED_EVM_PRINCIPAL=$EVM_PRINCIPAL
      WRAPPED_BRIDGE_CONTRACT=$(deploy_bft_bridge $WRAPPED_EVM_PRINCIPAL $MINTER_ADDRESS $WALLET)
      echo "Wrapped EVM Principal: $WRAPPED_EVM_PRINCIPAL"
      deploy_erc20_minter "$IC_NETWORK" "$INSTALL_MODE" "$EVM_PRINCIPAL" "$WRAPPED_EVM_PRINCIPAL" "$BASE_BRIDGE_CONTRACT" "$WRAPPED_BRIDGE_CONTRACT" "$SIGNING_STRATEGY" "$LOG_SETTINGS"
      ;;
    
    "rune-bridge")
      assert_isset_param "$INDEXER_URL" "INDEXER_URL"
      deploy_rune_bridge "$IC_NETWORK" "$INSTALL_MODE" "$BITCOIN_NETWORK" "$EVM_PRINCIPAL" "$OWNER" "$INDEXER_URL" "$SIGNING_STRATEGY" "$LOG_SETTINGS"
      ;;

    "btc-bridge")
      CKBTC_LEDGER=$(deploy_ckbtc_ledger)
      echo "CKBTC Ledger: $CKBTC_LEDGER"
      deploy_ckbtc_kyt
      CKBTC_MINTER=$(deploy_ckbtc_minter)
      echo "CKBTC Minter: $CKBTC_MINTER"
      deploy_btc_bridge "$IC_NETWORK" "$INSTALL_MODE" "$BITCOIN_NETWORK" "$OWNER" "$EVM_PRINCIPAL" "$CKBTC_MINTER" "$CKBTC_LEDGER" "$SIGNING_STRATEGY" "$LOG_SETTINGS"
      ;;

    *)
      echo "Unknown canister: $canister"
      exit 1
      ;;
  esac
done

start_icx "$EVM_PRINCIPAL"
