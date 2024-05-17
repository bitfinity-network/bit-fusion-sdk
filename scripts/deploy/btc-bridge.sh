#!/bin/bash

source "$(dirname "$0")/deploy_functions.sh"

IC_NETWORK="local"
BITCOIN_NETWORK="regtest"
CKBTC_LEDGER="mxzaz-hqaaa-aaaar-qaada-cai"
CKBTC_MINTER="mqygn-kiaaa-aaaar-qaadq-cai"

function usage() {
  echo "Usage: $0 [options]"
  echo "Options:"
  echo "  -h, --help                                      Display this help message"
  echo "  -b, --bitcoin-network <network>                 Bitcoin network (regtest, testnet, mainnet) (default: regtest)"
  echo "  -e, --evm-principal <principal>                 EVM Principal"
  echo "  -i, --ic-network <network>                      Internet Computer network (local, ic) (default: local)"
  echo "  -m, --install-mode <mode>                       Install mode (create, init, reinstall, upgrade)"
  echo "  --ckbtc-minter <canister-id>                    CK-BTC minter canister ID"
  echo "  --ckbtc-ledger <canister-id>                    CK-BTC ledger canister ID"
}

ARGS=$(getopt -o e:b:i:m:h --long evm-principal,ic-network,install-mode,bitcoin-network,ckbtc-ledger,ckbtc-minter,help -- "$@")
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

assert_isset_param "$INSTALL_MODE" "INSTALL_MODE"
assert_isset_param "$BITCOIN_NETWORK" "BITCOIN_NETWORK"
if [ "$IC_NETWORK" != "local" ]; then
  assert_isset_param "$EVM_PRINCIPAL" "EVM_PRINCIPAL"
fi

LOG_SETTINGS="opt record { enable_console=false; in_memory_records=opt 10000; log_filter=opt \"info,evm_core=debug,evm=debug\"; }"
OWNER=$(dfx identity get-principal)
SIGNING_STRATEGY="variant { ManagementCanister = record { key_id = variant { Production }; } }"

if [ "$IC_NETWORK" = "local" ]; then
  start_dfx 1
  SIGNING_STRATEGY="variant { ManagementCanister = record { key_id = variant { Dfx }; } }"
  EVM_PRINCIPAL=$(deploy_evm_testnet)
  CKBTC_LEDGER=$(deploy_ckbtc_ledger)
  echo "CKBTC Ledger: $CKBTC_LEDGER"
  deploy_ckbtc_kyt
  CKBTC_MINTER=$(deploy_ckbtc_minter)
  echo "CKBTC Minter: $CKBTC_MINTER"
fi

if [ "$INSTALL_MODE" = "create" ] || [ "$INSTALL_MODE" = "init" ]; then
  INSTALL_MODE="install"
fi 

if [ "$INSTALL_MODE" != "install" ] && [ "$INSTALL_MODE" != "upgrade" ] && [ "$INSTALL_MODE" != "reinstall" ]; then
  echo "Usage: $0 <create|init|upgrade|reinstall>"
  exit 1
fi

set -e
deploy_btc_bridge "$IC_NETWORK" "$INSTALL_MODE" "$BITCOIN_NETWORK" "$OWNER" "$EVM_PRINCIPAL" "$CKBTC_MINTER" "$CKBTC_LEDGER" "$SIGNING_STRATEGY" "$LOG_SETTINGS"
set +e

if [ "$IC_NETWORK" == "local" ]; then
  start_icx "$EVM_PRINCIPAL"
fi
