#!/bin/bash
FILE="dfx.json"

set -e

args=("$@")
# Install mode
INSTALL_MODE=${args[0]:-"unset"}
# Network
CHAIN_ID=${args[1]:-355113}
NETWORK=${args[2]:-"ic"}
# Wallet
WALLET=${args[3]:-"4cfzs-sqaaa-aaaak-aegca-cai"}

source ./scripts/dfx/deploy_functions.sh

entry_point() {
    dfx identity use EVM_DEPLOYER
    dfx identity --network="$NETWORK" set-wallet "$WALLET"

    LOG_SETTINGS="opt record { enable_console=false; in_memory_records=opt 1024; log_filter=opt \"off\"; }"
    OWNER=$(dfx identity get-principal)

    if [ "$INSTALL_MODE" = "create" ]; then
        create "$NETWORK"
        INSTALL_MODE="install"
        deploy "$NETWORK" "$INSTALL_MODE" "$LOG_SETTINGS" "$OWNER" "$CHAIN_ID" "$EVM_CANISTER_ID"

    elif [ "$INSTALL_MODE" = "upgrade" ] || [ "$INSTALL_MODE" = "reinstall" ]; then
        deploy "$NETWORK" "$INSTALL_MODE" "$LOG_SETTINGS" "$OWNER" "$CHAIN_ID" "$EVM_CANISTER_ID"
    else
        echo "Command Not Found!"
        exit 1
    fi
}

FILE="dfx.json"

# Create a backup of the original file
cp "$FILE" "$FILE.bak"

# Define a function to restore the backup file
restore_backup() {
    cp "$FILE.bak" "$FILE" && rm "$FILE.bak"
}

trap restore_backup EXIT INT TERM

# Update dfx.json
if [ "$NETWORK" = "ic" ]; then
    jq 'del(.canisters.evm_testnet) | del(.canisters.signature_verification) | del(.canisters.token)' $FILE >tmp.$$.json && mv tmp.$$.json $FILE
fi

entry_point
