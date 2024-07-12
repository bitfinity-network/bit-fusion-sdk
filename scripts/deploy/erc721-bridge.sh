#!/bin/bash

set -e
set -x

source "$(dirname "$0")/deploy_functions.sh"

CREATE_BFT_BRIDGE_TOOL="cargo run -q -p bridge-tool --"

function usage() {
  echo "Usage: $0 [options]"
  echo "Options:"
  echo "  -h, --help                                      Display this help message"
  echo "  -e, --evm-canister <principal>                  EVM canister principal"
  echo "  -w, --wallet <ETH wallet address>               Ethereum wallet address for deploy"
  echo "  --minter-address <minter-address>               Bridge minter address"
  echo "  --dfx-setup                                     Setup dfx locally"
}

ARGS=$(getopt -o e:w:m:h --long evm-canister,wallet,minter-address,dfx-setup,help -- "$@")
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

assert_isset_param "$MINTER_ADDRESS" "MINTER_ADDRESS"
if [ $DFX_SETUP -eq 1 ]; then
  start_dfx
  EVM_PRINCIPAL=$(deploy_evm_testnet)
else
  assert_isset_param "$EVM_PRINCIPAL" "EVM_PRINCIPAL"
fi

if [ -z "$WALLET" ]; then
  # get wallet
  WALLET=$(get_wallet $EVM_PRINCIPAL)
  echo "ETH wallet address: $WALLET"
fi

BRIDGE_ADDRESS=$(deploy_erc721_bridge "$EVM_PRINCIPAL" "$MINTER_ADDRESS" "$WALLET")

echo "ERC721 bridge ETH address: $BRIDGE_ADDRESS"
