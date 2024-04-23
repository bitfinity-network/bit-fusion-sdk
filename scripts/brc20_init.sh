#!/bin/bash

# Script to init Bitcoin daemon and dfx in regtest mode. The goal is to allow us to monitor logs in a separate terminal.
#
# NOTE: Version 0.18 of dfx has a bug not allowing BTC operations to work properly. Future versions may fix the issue.
# Until then, this script uses dfx version 0.17.
set +e

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
# dfx start --background --clean --enable-bitcoin >dfx_log.txt 2>&1
dfx start --clean --enable-bitcoin 2>&1 | grep -v "\[Canister g4xu7-jiaaa-aaaan-aaaaq-cai\]"
