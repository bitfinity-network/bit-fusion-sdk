#!/bin/bash

source "$(dirname "$0")/deploy_functions.sh"

IC_NETWORK="local"
BITCOIN_NETWORK="regtest"

function usage() {
  echo "Usage: $0 [options]"
  echo "Options:"
  echo "  -e, --evm-rpc-url <url>                   EVM RPC URL"
  echo "  -b, --bitcoin-network <network>           Bitcoin network (regtest, testnet, mainnet)"
  echo "  -i, --ic-network <network>                Internet Computer network (local, ic)"
  echo "  -m, --install-mode <mode>                 Install mode (create, init, reinstall, upgrade)"
  echo "  --indexer-url <url>                       Indexer URL"
  echo "  --rune-name <name>                        Rune name"
  echo "  --rune-block <block>                      Rune block"
  echo "  --rune-tx-id <tx-id>                      Rune transaction ID"
  echo "  --base-evm-link <canister-id>             Base EVM link canister ID"
  echo "  --wrapped-evm-link <canister-id>          Wrapped EVM link canister ID"
  echo "  --base-bridge-contract <canister-id>      Base bridge contract canister ID"
  echo "  --wrapped-bridge-contract <canister-id>   Wrapped bridge contract canister ID"
  echo "  -h, --help                                Display this help message"
}

ARGS=$(getopt -o e:b:i:m:h --long evm-rpc-url,bitcoin-network,ic-network,install-mode,base-evm-link,wrapped-evm-link,base-bridge-contract,wrapped-bridge-contract,rune-name,rune-block,rune-tx-id,indexer-url,help -- "$@")
while true; do
  case "$1" in
    -e|--evm-rpc-url)
      EVM_RPC_URL="$2"
      shift 2
      ;;

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

    --rune-name)
      RUNE_NAME="$2"
      shift 2
      ;;

    --rune-block)
      RUNE_BLOCK="$2"
      shift 2
      ;;
    
    --rune-tx-id)
      RUNE_TX_ID="$2"
      shift 2
      ;;

    --indexer-url)
      INDEXER_URL="$2"
      shift 2
      ;;

    --base-evm-link)
      BASE_EVM_LINK="$2"
      shift 2
      ;;

    --wrapped-evm-link)
      WRAPPED_EVM_LINK="$2"
      shift 2
      ;;

    --base-bridge-contract)
      BASE_BRIDGE_CONTRACT="$2"
      shift 2
      ;;

    --wrapped-bridge-contract)
      WRAPPED_BRIDGE_CONTRACT="$2"
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

if [ -z "$EVM_RPC_URL" ]; then
  echo "EVM_RPC_URL is required"
  usage
  exit 1
fi

# get positional arguments; skip $0, if empty 'all'
CANISTERS_TO_DEPLOY="${@:1}"
if [ -z "$CANISTERS_TO_DEPLOY" ]; then
  CANISTERS_TO_DEPLOY="icrc2-minter erc20-minter rune-bridge"
fi

echo "Deploying canisters: $CANISTERS_TO_DEPLOY"
exit 0

start_dfx() {
    echo "Attempting to create Alice's Identity"
    set +e

    if [ "$INSTALL_MODE" = "create" ]; then
        echo "Stopping DFX"
        dfx stop
        echo "Starting DFX"
        dfx start --clean --background --artificial-delay 0
    else
        return
    fi

    # Create identity
    dfx identity new --storage-mode=plaintext alice
    dfx identity use alice
    echo "Alice's Identity Created"
}

start_icx() {
    killall icx-proxy
    sleep 2
    # Start ICX Proxy
    dfx_local_port=$(dfx info replica-port)
    icx-proxy --fetch-root-key --address 127.0.0.1:8545 --dns-alias 127.0.0.1:$evm_id --replica http://localhost:$dfx_local_port &
    sleep 2

    curl -X POST -H "Content-Type: application/json" --data '{"jsonrpc": "2.0", "method": "eth_chainId", "params": [], "id":1}' 'http://127.0.0.1:8545'
}

start_dfx

LOG_SETTINGS="opt record { enable_console=true; in_memory_records=opt 10000; log_filter=opt \"error,did=debug,evm_core=debug,evm=debug\"; }"
OWNER=$(dfx identity get-principal)
SIGNING_STRATEGY="variant { 
  Local = record {
    private_key = blob \"\\01\\23\\45\\67\\89\\01\\23\\45\\67\\01\\01\\23\\45\\67\\89\\01\\23\\45\\67\\01\\01\\23\\45\\67\\89\\01\\23\\45\\67\\01\\67\\01\";
  }
}"

if [ "$INSTALL_MODE" = "create" ] || [ "$INSTALL_MODE" = "init" ]; then
  INSTALL_MODE="install"
fi

if [ "$INSTALL_MODE" != "install" ] && [ "$INSTALL_MODE" != "upgrade" ] && [ "$INSTALL_MODE" != "reinstall" ]; then
  echo "Usage: $0 <create|init|upgrade|reinstall>"
  exit 1
fi

for canister in $CANISTERS_TO_DEPLOY; do
  case $canister in

    *)
      echo "Unknown canister: $canister"
      exit 1
      ;;
  esac
done

start_icx
