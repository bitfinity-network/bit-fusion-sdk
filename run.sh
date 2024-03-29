#!/bin/bash

set -e

echo "Starting the Bitcoin daemon in regtest mode..."
./scripts/init.sh

echo "Starting the Internet Computer's local replica in the background..."
dfx start --clean --enable-bitcoin >dfx_log.txt 2>&1 &
DFX_START_PID=$!

# Wait a bit to ensure dfx has started
echo "Waiting for dfx to start..."
sleep 5

# Optional: Monitor specific logs from dfx start output
# Tail in background to keep showing new lines from dfx_log.txt
# Use grep or other tools to filter for specific log messages if needed
# tail -f dfx_log.txt | grep -v "\[Canister g4xu7-jiaaa-aaaan-aaaaq-cai\]" &

echo "Building all canisters..."
./scripts/build.sh

# Step 3: Deploy with init argument
echo "Deploying with 'init'... Other options <create | reinstall | upgrade>"
./scripts/deploy.sh init

echo "Deployment process complete."
