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

entry_point "$CHAIN_ID"
