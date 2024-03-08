#!/bin/bash
set -e

WASM_DIR=.artifact

create() {
    # Create canisters
    NETWORK=$1

    dfx canister --network=$NETWORK create --with-cycles=600000000000 --all
}

deploy() {
    set -e

    NETWORK=$1
    INSTALL_MODE=$2

    dfx build --network=$NETWORK

    deploy_inscriber_canister "$NETWORK" "$INSTALL_MODE"

    inscriber_id=$(dfx canister --network=$NETWORK id inscriber)

    echo "Inscriber ($inscriber_id) canister initialized."

}

deploy_inscriber_canister() {
    NETWORK=$1
    INSTALL_MODE=$2

    dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK inscriber
}
