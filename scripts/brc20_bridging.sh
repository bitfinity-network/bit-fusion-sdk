#!/bin/bash

# Script to deploy the Inscriber and Brc20Bridge canisters to perform a complete bridging flow (BRC20 <> ERC20).
#
# NOTE: Version 0.18 of dfx has a bug not allowing BTC operations to work properly. Future versions may fix the issue.
# Until then, this script uses dfx version 0.17.
set +e

CHAIN_ID=355113
GENERAL_INDEXER_URL="https://blockstream.info"
ORDINALS_INDEXER_URL="https://api.hiro.so/ordinals/v1/brc-20/tokens"

######################## Start the Bitcoin Daemon #####################

# Start bitcoind 
echo "Starting the Bitcoin daemon"
COMPOSE_FILE="../ckERC20/ord-testnet/bridging-flow/docker-compose.yml"

docker compose -f $COMPOSE_FILE up bitcoind -d
sleep 2

# Check if the wallet already exists
wallet_exists=$(docker compose -f $COMPOSE_FILE exec -T bitcoind bitcoin-cli -regtest listwallets | grep -q "testwallet" && echo "exists" || echo "not exists")

if [ "$wallet_exists" = "not exists" ]; then
    docker compose -f $COMPOSE_FILE exec -T bitcoind bitcoin-cli -regtest createwallet "testwallet"
fi

load_output=$(docker compose -f $COMPOSE_FILE exec -T bitcoind bitcoin-cli -regtest loadwallet "testwallet" 2>&1)

if echo "$load_output" | grep -q "Wallet file verification failed"; then
    echo "Wallet already exists but could not be loaded due to verification failure."
elif echo "$load_output" | grep -q "Wallet loaded successfully"; then
    echo "Wallet loaded successfully."
else
    echo "Unexpected wallet load output: $load_output"
fi

# Generate 101 blocks to make sure we have some coins to spend
height=$(docker compose -f $COMPOSE_FILE exec -T bitcoind bitcoin-cli -regtest getblockcount)
if [ $height -lt 101 ]; then
    docker compose -f $COMPOSE_FILE exec -T bitcoind bitcoin-cli -regtest generate 101
fi

# Start the ord service.
docker compose -f $COMPOSE_FILE up ord -d
if [ $? -ne 0 ]; then
    echo "Failed to start 'ord' service. Attempting to continue script..."
fi

############################### Configure Dfx #################################

echo "Starting dfx in a clean state"
dfx stop
dfx start --background --clean --enable-bitcoin >dfx_log.txt 2>&1

dfx identity new --force brc20-admin
dfx identity use brc20-admin

ADMIN_PRINCIPAL=$(dfx identity get-principal)
ADMIN_WALLET=$(dfx identity get-wallet)

######################### Deploy the Inscriber Canister ######################

echo "Deploying the Inscriber canister"
dfx canister create inscriber

INSCRIBER=$(dfx canister id inscriber)

dfx deploy inscriber --argument "(record {
    network = variant { regtest };
    logger = record {
        enable_console = true;
        in_memory_records = opt 10000;
        log_filter = opt \"info\";
    };
})"

######################### Deploy EVM and BRC20 Bridge ######################

echo "Deploying EVMc testnet"
dfx canister create evm_testnet
EVM=$(dfx canister id evm_testnet)

dfx deploy signature_verification --argument "(vec { principal \"${EVM}\" })"
SIGNATURE_VERIFICATION=$(dfx canister id signature_verification)

dfx deploy evm_testnet --argument "(record {
    min_gas_price = 10;
    signature_verification_principal = principal \"${SIGNATURE_VERIFICATION}\";
    log_settings = opt record {
        enable_console = true;
        in_memory_records = opt 10000;
        log_filter = opt \"error,did=debug,evm_core=debug,evm=debug\";
    };
    owner = principal \"${ADMIN_PRINCIPAL}\";
    genesis_accounts = vec { };
    chain_id = $CHAIN_ID;
    coinbase = \"0x0000000000000000000000000000000000000000\";
})"

echo "Deploying BRC20 bridge"
dfx deploy brc20-bridge --argument "(record {
    general_indexer = \"${GENERAL_INDEXER_URL}\";
    erc20_minter_fee = 10;
    admin = principal \"${ADMIN_PRINCIPAL}\";
    signing_strategy = variant { ManagementCanister = record { key_id = variant { Dfx } } };
    inscriber = principal \"${INSCRIBER}\";
    evm_link = variant { Ic = principal \"${EVM}\" };
    network = variant { regtest };
    logger = record {
       enable_console = true;
       in_memory_records = opt 10000;
       log_filter = opt \"trace\";
    };
    ordinals_indexer = \"${ORDINALS_INDEXER_URL}\";
})"

######################## Deploy BFT and Token Contracts ######################

ETH_WALLET=$(cargo run -q -p create_bft_bridge_tool -- create-wallet --evm-canister="$EVM")
ETH_WALLET_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- wallet-address --wallet="$ETH_WALLET")
ETH_WALLET_CANDID=$(cargo run -q -p create_bft_bridge_tool -- wallet-address --wallet="$ETH_WALLET" --candid)

BRC20_BRIDGE=$(dfx canister id brc20-bridge)

res=$(dfx canister call brc20-bridge get_evm_address)
res=${res#*\"}
BRC20_BRIDGE_ETH_ADDRESS=${res%\"*}

echo "BRC20 bridge eth address: ${BRC20_BRIDGE_ETH_ADDRESS}"

echo "Minting ETH tokens for BRC20 bridge canister"
dfx canister call evm_testnet mint_native_tokens "(\"${BRC20_BRIDGE_ETH_ADDRESS}\", \"340282366920938463463374607431768211455\")"

BFT_ETH_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- deploy-bft-bridge --minter-address="$BRC20_BRIDGE_ETH_ADDRESS" --evm="$EVM" --wallet="$ETH_WALLET")
echo "BFT ETH address: $BFT_ETH_ADDRESS"

TOKEN_ETH_ADDRESS=$(cargo run -q -p create_bft_bridge_tool -- create-token \
  --bft-bridge-address="$BFT_ETH_ADDRESS" \
  --token-name=BRC20 \
  --token-id="$BRC20_BRIDGE" \
  --evm-canister="$EVM" \
  --wallet="$ETH_WALLET")

echo "Wrapped token ETH address: $TOKEN_ETH_ADDRESS"

echo "Configuring BRC20 bridge canister"
dfx canister call brc20-bridge admin_configure_bft_bridge "(record {
  decimals = 0;
  token_symbol = vec { 42; 54; 43; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; };
  token_address = \"$TOKEN_ETH_ADDRESS\";
  bridge_address = \"$BFT_ETH_ADDRESS\";
  erc20_chain_id = 355113;
  token_name = vec { 42; 54; 43; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; 0; };
})"

echo "All canisters successfully deployed."
