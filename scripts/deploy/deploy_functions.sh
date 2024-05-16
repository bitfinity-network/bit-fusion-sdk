#!/bin/bash

CREATE_BFT_BRIDGE_TOOL="cargo run -q -p create_bft_bridge_tool --"

link_to_variant() {
  LINK=$1
  if [[ $URL == "http"* ]]; then
    IC_TYPE="variant { Http = \"${LINK}\" }"
  else
    IC_TYPE="variant { Ic = principal \"${LINK}\" }"
  fi
  echo "$IC_TYPE"
}

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
    evm_principal = principal \"$EVM_PRINCIPAL\";
    signing_strategy = $SIGNING_STRATEGY;
    log_settings = opt $LOG_SETTINGS;
    owner = principal \"$ADMIN_PRINCIPAL\";
  })"

  echo "deploying icrc2-minter with args: $args"

  dfx canister install --mode=$INSTALL_MODE --wasm=./.artifact/icrc2-minter.wasm.gz --yes --network=$NETWORK --argument="$args" icrc2-minter
}

deploy_erc20_minter() {
  NETWORK="$1"
  INSTALL_MODE="$2"
  BASE_EVM_LINK=$(link_to_variant "$3")
  WRAPPED_EVM_LINK=$(link_to_variant "$4")
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
    log_settings = opt $LOG_SETTINGS;
  })"

  echo "deploying erc20-minter with args: $args"

  dfx canister install --mode=$INSTALL_MODE --yes --wasm=./.artifact/erc20-minter.wasm.gz --network=$NETWORK --argument="$args" erc20-minter
}

deploy_rune_bridge() {
  NETWORK="$1"
  INSTALL_MODE="$2"
  BITCOIN_NETWORK="$3"
  EVM_LINK=$(link_to_variant "$4")
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
    log_settings = opt $LOG_SETTINGS;
    min_confirmations = 1;
    indexer_url = \"$INDEXER_URL\";
  })"

  echo "deploying rune-bridge with args: $args"
  
  dfx canister install --mode=$INSTALL_MODE --yes --wasm=./.artifact/rune-bridge.wasm.gz --network=$NETWORK --argument="$args" rune-bridge
}

deploy_btc_bridge() {
  NETWORK="$1"
  INSTALL_MODE="$2"
  BITCOIN_NETWORK="$3"
  ADMIN_PRINCIPAL="$4"
  EVM_LINK=$(link_to_variant "$5")
  CKBTC_MINTER="$6"
  CKBTC_LEDGER="$7"
  SIGNING_STRATEGY="$8"
  LOG_SETTINGS="$9"
  LEDGER_FEE="${10:-10}"

  create_canister $NETWORK btc-bridge

  args="(record {
    ck_btc_ledger = principal \"$CKBTC_LEDGER\";
    ck_btc_minter = principal \"$CKBTC_MINTER\";
    network = variant { $BITCOIN_NETWORK };
    evm_link = $EVM_LINK;
    signing_strategy = $SIGNING_STRATEGY;
    admin = principal \"$ADMIN_PRINCIPAL\";
    ck_btc_ledger_fee = $LEDGER_FEE;
    log_settings = $LOG_SETTINGS;
  })"

  echo "deploying btc-bridge with args: $args"

  dfx canister install --mode=$INSTALL_MODE --yes --wasm=./.artifact/btc-bridge.wasm.gz --network=$NETWORK --argument="$args" btc-bridge
}

get_wallet() {
  EVM_PRINCIPAL="$1"
  ETH_WALLET=$($CREATE_BFT_BRIDGE_TOOL create-wallet --evm-canister="$EVM_PRINCIPAL")

  echo "$ETH_WALLET"
}

deploy_bft_bridge() {
  EVM_PRINCIPAL="$1"
  MINTER_ADDRESS="$2"
  WALLET="$3"

  BRIDGE_ADDRESS=$($CREATE_BFT_BRIDGE_TOOL deploy-bft-bridge --minter-address="$MINTER_ADDRESS" --evm="$EVM_PRINCIPAL" --wallet="$WALLET")

  echo "$BRIDGE_ADDRESS"
}

deploy_erc721_bridge() {
  EVM_PRINCIPAL="$1"
  MINTER_ADDRESS="$2"
  WALLET="$3"

  BRIDGE_ADDRESS=$($CREATE_BFT_BRIDGE_TOOL deploy-erc721-bridge --minter-address="$MINTER_ADDRESS" --evm="$EVM_PRINCIPAL" --wallet="$WALLET")

  echo "$BRIDGE_ADDRESS"
}
