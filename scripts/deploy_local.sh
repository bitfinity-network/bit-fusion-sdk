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

source ./scripts/deploy_functions.sh

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

entry_point() {
    CHAIN_ID=$1
    OWNER=$(dfx identity get-principal)

    if [ "$INSTALL_MODE" = "create" ] || [ "$INSTALL_MODE" = "init" ]; then
        create "$NETWORK"
        INSTALL_MODE="install"
        deploy "$NETWORK" "$INSTALL_MODE" "$OWNER" "$CHAIN_ID"

    elif [ "$INSTALL_MODE" = "upgrade" ] || [ "$INSTALL_MODE" = "reinstall" ]; then
        deploy "$NETWORK" "$INSTALL_MODE" "$OWNER" "$CHAIN_ID"
    else
        echo "Usage: $0 <create|init|upgrade|reinstall>"
        exit 1
    fi
}

start_dfx

entry_point "$CHAIN_ID"
