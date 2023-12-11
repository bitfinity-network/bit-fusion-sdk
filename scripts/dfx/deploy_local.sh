#!/bin/bash

FILE="dfx.json"

set -e
set -x

args=("$@")
# Install mode
INSTALL_MODE=${args[0]:-"unset"}
CHAIN_ID=${args[1]:-355113}
# Network
NETWORK="local"

NETWORK_NAME="testnet"

FILE="dfx.json"

source ./scripts/dfx/deploy_functions.sh

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

entry_point() {
    CHAIN_ID=$1
    LOG_SETTINGS="opt record { enable_console=true; in_memory_records=opt 2048; log_filter=opt \"error,did=debug,evm_core=debug,evm=debug,minter_canister=debug\"; }"
    OWNER=$(dfx identity get-principal)

    if [ "$INSTALL_MODE" = "create" ] || [ "$INSTALL_MODE" = "init" ]; then
        create "$NETWORK"
        INSTALL_MODE="install"
        deploy "$NETWORK" "$INSTALL_MODE" "$LOG_SETTINGS" "$OWNER" "$CHAIN_ID"

    elif [ "$INSTALL_MODE" = "upgrade" ] || [ "$INSTALL_MODE" = "reinstall" ]; then
        deploy "$NETWORK" "$INSTALL_MODE" "$LOG_SETTINGS" "$OWNER" "$CHAIN_ID"
    else
        echo "Usage: $0 <create|init|upgrade|reinstall>"
        exit 1
    fi
}

start_dfx

entry_point "$CHAIN_ID"

start_icx
