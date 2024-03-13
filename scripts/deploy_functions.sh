#!/bin/bash
set -e

WASM_DIR=.artifact

create() {
    # Create canisters
    NETWORK=$1

    dfx canister --network=$NETWORK create --with-cycles=6000000000000 --all
}

deploy_inscriber_canister() {
    NETWORK=$1
    INSTALL_MODE=$2

    inscriber_init_args="(variant { regtest })"

    echo "Deploying Inscriber canister with init args: $inscriber_init_args"

    dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK --argument="$inscriber_init_args" inscriber
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
