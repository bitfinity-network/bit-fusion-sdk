#!/bin/bash

source "$(dirname "$0")/deploy_functions.sh"

CREATE_BFT_BRIDGE_TOOL="cargo run -q -p create_bft_bridge_tool --"
BFT_BRIDGE="bft-bridge"
ERC721_BRIDGE="erc721-bridge"

function usage() {
  echo "Usage: $0 [options] [bridge]..."
  echo "Bridge: $BFT_BRIDGE $ERC721_BRIDGE"
  echo "Options:"
  echo "  -h, --help                                      Display this help message"
  echo "  -e, --evm-canister <principal>                  EVM canister principal"
  echo "  -w, --wallet <ETH wallet address>               Ethereum wallet address for deploy"
  echo "  --minter-address <minter-address>               Bridge minter address"
}

ARGS=$(getopt -o e:w:m:h --long evm-canister,wallet,minter-address,help -- "$@")
while true; do
  case "$1" in

    -w|--wallet)
      WALLET="$2"
      shift 2
      ;;

    -e|--evm-canister)
      EVM_PRINCIPAL="$2"
      shift 2
      ;;

    -m|--minter-address)
      MINTER_ADDRESS="$2"
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

assert_isset_param () {
  PARAM=$1
  NAME=$2
  if [ -z "$PARAM" ]; then
    echo "$NAME is required"
    usage
    exit 1
  fi
}

# get positional arguments; skip $0, if empty 'all'
BRIDGE_TO_DEPLOY="${@:1}"
if [ -z "$BRIDGE_TO_DEPLOY" ]; then
  BRIDGE_TO_DEPLOY="$BFT_BRIDGE"
fi

assert_isset_param "$EVM_PRINCIPAL" "EVM_PRINCIPAL"
assert_isset_param "$MINTER_ADDRESS" "MINTER_ADDRESS"

if [ -z "$WALLET" ]; then
  # get wallet
  WALLET=$(get_wallet $EVM_PRINCIPAL)
  echo "ETH wallet address: $WALLET"
fi

for bridge in $BRIDGE_TO_DEPLOY; do
  case $bridge in
    $BFT_BRIDGE)
      BRIDGE_ADDRESS=$(deploy_bft_bridge "$EVM_PRINCIPAL" "$MINTER_ADDRESS" "$WALLET")
      echo "BFT bridge ETH address: $BRIDGE_ADDRESS"
      ;;

    $ERC721_BRIDGE)
      BRIDGE_ADDRESS=$(deploy_erc721_bridge "$EVM_PRINCIPAL" "$MINTER_ADDRESS" "$WALLET")
      echo "ERC721 bridge ETH address: $BRIDGE_ADDRESS"
      ;;
    
    *)
      echo "Unknown bridge: $bridge"
      usage
      exit 1
      ;;
  esac
done
