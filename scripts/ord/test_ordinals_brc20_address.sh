#!/usr/bin/env sh

# Generates sample address for receiving BRC20 tokens.
# Be sure to set `TO_ADDRESS` env after run this.

bitcoin_cli="docker exec bitcoind bitcoin-cli -regtest"
ord_wallet="docker exec ord ./ord --regtest --bitcoin-rpc-url bitcoind:18443 wallet --server-url http://localhost:8000"

TO_ADDRESS=$($ord_wallet receive | jq -r ".address")
$bitcoin_cli generatetoaddress 101 $TO_ADDRESS
echo $TO_ADDRESS
