#!/bin/bash

create_canister() {
    # Create canisters
    NETWORK=$1
    CANISTER=$2

    dfx canister --network=$NETWORK create --with-cycles=600000000000 $CANISTER
}

deploy_icrc2_minter() {
  NETWORK="$1"
  INSTALL_MODE="$2"
  EVM_PRINCIPAL="$3"
  ADMIN_PRINCIPAL="$4"
  SIGNING_STRATEGY="$5"
  LOG_SETTINGS="$6"

  create_canister $NETWORK icrc2-minter

  args="(record {
    evm_principal = principal \"$EVM\";
    signing_strategy = $SIGNING_STRATEGY;
    log_settings = $LOG_SETTINGS;
    owner = principal \"$ADMIN_PRINCIPAL\";
  })"

  echo "deploying icrc2-minter with args: $args"

  dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK --arguments="$args" icrc2-minter
}

deploy_erc20_minter() {
  NETWORK="$1"
  INSTALL_MODE="$2"
  BASE_EVM_LINK="$3"
  WRAPPED_EVM_LINK="$4"
  BASE_BRIDGE_CONTRACT="$5"
  WRAPPED_BRIDGE_CONTRACT="$6"
  SIGNING_STRATEGY="$7"
  LOG_SETTINGS="$8"

  create_canister $NETWORK erc20-minter

  args="(record {
    base_evm_link = $BASE_EVM_LINK;
    wrapped_evm_link = $WRAPPED_EVM_LINK;
    base_bridge_contract = \"$BASE_BRIDGE_CONTRACT\";
    wrapped_bridge_contract = \"$WRAPPED_BRIDGE_CONTRACT\";
    signing_strategy = $SIGNING_STRATEGY;
    log_settings = $LOG_SETTINGS;
  })"

  echo "deploying erc20-minter with args: $args"

  dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK --arguments="$args" erc20-minter
}

deploy_rune_bridge() {
  NETWORK="$1"
  INSTALL_MODE="$2"
  BITCOIN_NETWORK="$3"
  EVM_LINK="$4"
  ADMIN_PRINCIPAL="$5"
  INDEXER_URL="$6"
  SIGNING_STRATEGY="$7"
  LOG_SETTINGS="$8"

  create_canister $NETWORK rune-bridge

  args="(record {
    network = variant { $BITCOIN_NETWORK };
    evm_link = $EVM_LINK;
    signing_strategy = $SIGNING_STRATEGY;
    admin = principal \"$ADMIN_PRINCIPAL\";
    log_settings = $LOG_SETTINGS;
    min_confirmations = 1;
    indexer_url = \"$INDEXER_URL\";
  })"

  echo "deploying rune-bridge with args: $args"
  
  dfx canister install --mode=$INSTALL_MODE --yes --network=$NETWORK --arguments="$args" rune-bridge
}
