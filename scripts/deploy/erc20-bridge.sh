#!/bin/bash

source "$(dirname "$0")/deploy_functions.sh"

IC_NETWORK="local"

function usage() {
  echo "Usage: $0 [options]"
  echo "Options:"
  echo "  -h, --help                                      Display this help message"
  echo "  -e, --evm-principal <principal>                 EVM Principal"
  echo "  -i, --ic-network <network>                      Internet Computer network (local, ic) (default: local)"
  echo "  -m, --install-mode <mode>                       Install mode (create, init, reinstall, upgrade)"
  echo "  --base-evm <canister-id>                        Base EVM link canister ID"
  echo "  --wrapped-evm <canister-id>                     Wrapped EVM link canister ID"
  echo "  --erc20-base-bridge-contract <canister-id>      ERC20 Base bridge contract canister ID"
  echo "  --erc20-wrapped-bridge-contract <canister-id>   ERC20 Wrapped bridge contract canister ID"
}

ARGS=$(getopt -o e:i:m:h --long evm-principal,ic-network,install-mode,base-evm,wrapped-evm,erc20-base-bridge-contract,erc20-wrapped-bridge-contract,help -- "$@")
while true; do
  case "$1" in
    -e|--evm-principal)
      EVM_PRINCIPAL="$2"
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

assert_isset_param "$INSTALL_MODE" "INSTALL_MODE"
if [ "$IC_NETWORK" != "local" ]; then
  assert_isset_param "$EVM_PRINCIPAL" "EVM_PRINCIPAL"
  assert_isset_param "$BASE_EVM_LINK" "BASE_EVM_LINK"
  assert_isset_param "$WRAPPED_EVM_LINK" "WRAPPED_EVM_LINK"
  assert_isset_param "$WRAPPED_BRIDGE_CONTRACT" "WRAPPED_BRIDGE_CONTRACT"
  assert_isset_param "$BASE_BRIDGE_CONTRACT" "BASE_BRIDGE_CONTRACT"
fi

# get positional arguments; skip $0, if empty 'all'
CANISTERS_TO_DEPLOY="${@:1}"
if [ -z "$CANISTERS_TO_DEPLOY" ]; then
  CANISTERS_TO_DEPLOY="icrc2-minter erc20-minter rune-bridge btc-bridge"
fi

LOG_SETTINGS="opt record { enable_console=false; in_memory_records=opt 10000; log_filter=opt \"info,evm_core=debug,evm=debug\"; }"
OWNER=$(dfx identity get-principal)
SIGNING_STRATEGY="variant { ManagementCanister = record { key_id = variant { Dfx }; } }"

if [ "$IC_NETWORK" = "local" ]; then
  start_dfx
  SIGNING_STRATEGY="variant { Local = record { private_key = blob \"\\01\\23\\45\\67\\89\\01\\23\\45\\67\\01\\01\\23\\45\\67\\89\\01\\23\\45\\67\\01\\01\\23\\45\\67\\89\\01\\23\\45\\67\\01\\67\\01\"; } }"
  EVM_PRINCIPAL=$(deploy_evm_testnet)
  ICRC2_MINTER_ID=$(dfx canister id icrc2-minter)
  if [ -z "$ICRC2_MINTER_ID" ]; then
    deploy_icrc2_minter "local" "install" "$EVM_PRINCIPAL" "$OWNER" "$SIGNING_STRATEGY" "$LOG_SETTINGS"
  fi
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
fi

if [ "$INSTALL_MODE" = "create" ] || [ "$INSTALL_MODE" = "init" ]; then
  INSTALL_MODE="install"
fi

if [ "$INSTALL_MODE" != "install" ] && [ "$INSTALL_MODE" != "upgrade" ] && [ "$INSTALL_MODE" != "reinstall" ]; then
  echo "Usage: $0 <create|init|upgrade|reinstall>"
  exit 1
fi

set -e
deploy_erc20_minter "$IC_NETWORK" "$INSTALL_MODE" "$EVM_PRINCIPAL" "$WRAPPED_EVM_PRINCIPAL" "$BASE_BRIDGE_CONTRACT" "$WRAPPED_BRIDGE_CONTRACT" "$SIGNING_STRATEGY" "$LOG_SETTINGS"
set +e

if [ "$IC_NETWORK" == "local" ]; then
  start_icx "$EVM_PRINCIPAL"
fi
