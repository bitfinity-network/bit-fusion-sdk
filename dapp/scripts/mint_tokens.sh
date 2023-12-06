#!/bin/bash

# Check if user address is provided
if [ -z "$1" ]; then
    echo "Usage: $0 <user_address>"
    exit 1
fi

# Set the user address and amount to mint
user_address="$1"
amount_to_mint="0x56BC75E2D63100000"  # Adjust the amount as needed

# Ethereum node endpoint
ethereum_node_url="http://127.0.0.1:8545"

# Construct the JSON-RPC request
json_rpc_request='{
  "jsonrpc":"2.0",
  "id":"1",
  "method":"ic_mintNativeToken",
  "params":["'$user_address'", "'$amount_to_mint'"]
}'

# Make the POST request using curl
curl "$ethereum_node_url" \
  -X POST \
  -H 'Content-Type: application/json' \
  -d "$json_rpc_request"
