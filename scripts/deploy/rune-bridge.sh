#!/bin/bash

set -e
set -x

source "$(dirname "$0")/deploy_functions.sh"

IC_NETWORK="local"
BITCOIN_NETWORK="regtest"
INDEXER_URL="https://127.0.0.1:8001"

function usage() {
  echo "Usage: $0 [options]"
  echo "Options:"
  echo "  -h, --help                                      Display this help message"
  echo "  -b, --bitcoin-network <network>                 Bitcoin network (regtest, testnet, mainnet) (default: regtest)"
  echo "  -e, --evm-principal <principal>                 EVM Principal"
  echo "  -i, --ic-network <network>                      Internet Computer network (local, ic) (default: local)"
  echo "  -m, --install-mode <mode>                       Install mode (create, init, reinstall, upgrade)"
  echo "  --indexer-url <url>                             Indexer URL"
}

ARGS=$(getopt -o e:b:i:m:h --long evm-principal,ic-network,install-mode,bitcoin-network,indexer-url,help -- "$@")
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

    -b|--bitcoin-network)
      BITCOIN_NETWORK="$2"
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

assert_isset_param "$INSTALL_MODE" "INSTALL_MODE"
assert_isset_param "$BITCOIN_NETWORK" "BITCOIN_NETWORK"
if [ "$IC_NETWORK" != "local" ]; then
  assert_isset_param "$EVM_PRINCIPAL" "EVM_PRINCIPAL"
fi

LOG_SETTINGS="opt record { enable_console=false; in_memory_records=opt 10000; log_filter=opt \"info,evm_core=debug,evm=info\"; }"
OWNER=$(dfx identity get-principal)
SIGNING_STRATEGY="variant { ManagementCanister = record { key_id = variant { Production }; } }"

if [ "$IC_NETWORK" = "local" ]; then
  start_dfx 1
  SIGNING_STRATEGY="variant { ManagementCanister = record { key_id = variant { Dfx }; } }"
  EVM_PRINCIPAL=$(deploy_evm_testnet)
fi

if [ "$INSTALL_MODE" = "create" ] || [ "$INSTALL_MODE" = "init" ]; then
  INSTALL_MODE="install"
fi

if [ "$INSTALL_MODE" != "install" ] && [ "$INSTALL_MODE" != "upgrade" ] && [ "$INSTALL_MODE" != "reinstall" ]; then
  echo "Usage: $0 <create|init|upgrade|reinstall>"
  exit 1
fi

deploy_rune_bridge "$IC_NETWORK" "$INSTALL_MODE" "$BITCOIN_NETWORK" "$EVM_PRINCIPAL" "$OWNER" "$INDEXER_URL" "$SIGNING_STRATEGY" "$LOG_SETTINGS"

if [ "$IC_NETWORK" == "local" ]; then
  start_icx "$EVM_PRINCIPAL"
fi
