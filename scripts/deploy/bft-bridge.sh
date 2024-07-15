#!/bin/bash

set -e
set -x

source "$(dirname "$0")/deploy_functions.sh"

CREATE_BFT_BRIDGE_TOOL="cargo run -q -p bridge-tool --"
DFX_SETUP=0
IS_WRAPPED=false

function usage() {
  echo "Usage: $0 [options]"
  echo "Options:"
  echo "  -h, --help                                      Display this help message"
  echo "  -e, --evm-canister <principal>                  EVM canister principal"
  echo "  -w, --wallet <ETH wallet address>               Ethereum wallet address for deploy"
  echo "  --minter-address <minter-address>               Bridge minter address"
  echo "  --fee-charge-address <fee-charge-address>       Bridge fee charge address"
  echo "  --is-wrapped                                     Is wrapped"
  echo "  --dfx-setup                                     Setup dfx locally"
}

ARGS=$(getopt -o e:w:m:h --long evm-canister,wallet,minter-address,fee-charge-address,is-wrapped,dfx-setup,help -- "$@")
while true; do
  case "$1" in

  -w | --wallet)
    WALLET="$2"
    shift 2
    ;;

  -e | --evm-canister)
    EVM_PRINCIPAL="$2"
    shift 2
    ;;

  -m | --minter-address)
    MINTER_ADDRESS="$2"
    shift 2
    ;;

  -f | --fee-charge-address)
    FEE_CHARGE_ADDRESS="$2"
    shift 2
    ;;

  --is-wrapped)
    IS_WRAPPED=true
    shift
    ;;

  --dfx-setup)
    DFX_SETUP=1
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

if [ $DFX_SETUP -eq 1 ]; then
  start_dfx
  EVM_PRINCIPAL=$(deploy_evm_testnet)
else
  assert_isset_param "$MINTER_ADDRESS" "MINTER_ADDRESS"
  assert_isset_param "$FEE_CHARGE_ADDRESS" "FEE_CHARGE_ADDRESS"
  assert_isset_param "$EVM_PRINCIPAL" "EVM_PRINCIPAL"
fi

if [ -z "$WALLET" ]; then
  # get wallet
  WALLET=$(get_wallet $EVM_PRINCIPAL)
  echo "ETH wallet address: $WALLET"
fi

BRIDGE_ADDRESS=$(deploy_bft_bridge "$EVM_PRINCIPAL" "$WALLET" "$MINTER_ADDRESS" "$FEE_CHARGE_ADDRESS" "$IS_WRAPPED")


echo "BFT bridge ETH address: $BRIDGE_ADDRESS"
