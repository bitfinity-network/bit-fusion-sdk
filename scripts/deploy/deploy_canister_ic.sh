#!/bin/bash

source "$(dirname "$0")/deploy_functions.sh"

IC_NETWORK="ic"
BITCOIN_NETWORK="mainnet"

function usage() {
  echo "Usage: $0 [options]"
  echo "Options:"
  echo "  -h, --help                                      Display this help message"
  echo "  -e, --evm-rpc-url <url>                         EVM RPC URL"
  echo "  -b, --bitcoin-network <network>                 Bitcoin network (regtest, testnet, mainnet)"
  echo "  -i, --ic-network <network>                      Internet Computer network (local, ic)"
  echo "  -m, --install-mode <mode>                       Install mode (create, init, reinstall, upgrade)"
  echo "  --indexer-url <url>                             Indexer URL"
  echo "  --base-evm <canister-id>                        Base EVM link canister ID"
  echo "  --wrapped-evm <canister-id>                     Wrapped EVM link canister ID"
  echo "  --erc20-base-bridge-contract <canister-id>      ERC20 Base bridge contract canister ID"
  echo "  --erc20-wrapped-bridge-contract <canister-id>   ERC20 Wrapped bridge contract canister ID"
  echo "  --ckbtc-minter <canister-id>                    CK-BTC minter canister ID"
  echo "  --ckbtc-ledger <canister-id>                    CK-BTC ledger canister ID"
}

ARGS=$(getopt -o e:b:i:m:h --long evm-rpc-url,bitcoin-network,ic-network,install-mode,ckbtc-ledger,ckbtc-minter,base-evm,wrapped-evm,erc20-base-bridge-contract,erc20-wrapped-bridge-contract,rune-name,rune-block,rune-tx-id,indexer-url,help -- "$@")
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

    --indexer-url)
      INDEXER_URL="$2"
      shift 2
      ;;

    --base-evm)
      BASE_EVM_LINK=$(link_to_variant "$2")
      shift 2
      ;;

    --wrapped-evm)
      WRAPPED_EVM_LINK=$(link_to_variant "$2")
      shift 2
      ;;

    --erc20-base-bridge-contract)
      BASE_BRIDGE_CONTRACT="$2"
      shift 2
      ;;

    --erc20-wrapped-bridge-contract)
      WRAPPED_BRIDGE_CONTRACT="$2"
      shift 2
      ;;

    --ckbtc-minter)
      CKBTC_MINTER="$2"
      shift 2
      ;;
    
    --ckbtc-ledger)
      CKBTC_LEDGER="$2"
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
  CANISTERS_TO_DEPLOY="icrc2-minter erc20-minter rune-bridge btc-bridge"
fi

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

LOG_SETTINGS="opt record { enable_console=false; in_memory_records=opt 10000; log_filter=opt \"info,evm_core=debug,evm=debug\"; }"
OWNER=$(dfx identity get-principal)
SIGNING_STRATEGY="variant { ManagementCanister = record { key_id = variant = { Dfx }; } }"

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
      assert_isset_param "$EVM_RPC_URL" "EVM_RPC_URL"
      deploy_icrc2_minter $IC_NETWORK $INSTALL_MODE $EVM_RPC_URL $OWNER $SIGNING_STRATEGY $LOG_SETTINGS
      ;;

    "erc20-minter")
      assert_isset_param "$BASE_EVM_LINK" "BASE_EVM_LINK"
      assert_isset_param "$WRAPPED_EVM_LINK" "WRAPPED_EVM_LINK"
      assert_isset_param "$BASE_BRIDGE_CONTRACT" "BASE_BRIDGE_CONTRACT"
      assert_isset_param "$WRAPPED_BRIDGE_CONTRACT" "WRAPPED_BRIDGE_CONTRACT"
      deploy_erc20_minter $IC_NETWORK $INSTALL_MODE $BASE_EVM_LINK $WRAPPED_EVM_LINK $BASE_BRIDGE_CONTRACT $WRAPPED_BRIDGE_CONTRACT $SIGNING_STRATEGY $LOG_SETTINGS
      ;;
    
    "rune-bridge")
      assert_isset_param "$BASE_EVM_LINK" "BASE_EVM_LINK"
      assert_isset_param "$INDEXER_URL" "INDEXER_URL"
      deploy_rune_bridge $IC_NETWORK $INSTALL_MODE $BITCOIN_NETWORK $BASE_EVM_LINK $OWNER $INDEXER_URL $SIGNING_STRATEGY $LOG_SETTINGS
      ;;

    "btc-bridge")
      assert_isset_param "$BASE_EVM_LINK" "BASE_EVM_LINK"
      assert_isset_param "$CKBTC_MINTER" "CKBTC_MINTER"
      assert_isset_param "$CKBTC_LEDGER" "CKBTC_LEDGER"
      deploy_btc_bridge "$IC_NETWORK" "$INSTALL_MODE" "$BITCOIN_NETWORK" "$OWNER" "$EVM_PRINCIPAL" "$CKBTC_MINTER" "$CKBTC_LEDGER" "$SIGNING_STRATEGY" "$LOG_SETTINGS"
      ;;

    *)
      echo "Unknown canister: $canister"
      exit 1
      ;;
  esac
done

start_icx
