#!/bin/bash

if [ -z "$1" ]; then
    echo "Usage: $0 <user_address>"
    exit 1
fi

user_address="$1"
amount_to_mint="0x56BC75E2D63100000"  

ethereum_node_url="http://127.0.0.1:8545"

json_rpc_request='{
  "jsonrpc":"2.0",
  "id":"1",
  "method":"ic_mintNativeToken",
  "params":["'$user_address'", "'$amount_to_mint'"]
}'

curl "$ethereum_node_url" \
  -X POST \
  -H 'Content-Type: application/json' \
  -d "$json_rpc_request"
