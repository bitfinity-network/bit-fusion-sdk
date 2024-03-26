#!/usr/bin/env sh

# Generates sample address for receiving BRC20 tokens.
# Be sure to set `TO_ADDRESS` env after run this.

TO_ADDRESS=$(docker exec -it ord-cli ord --regtest --bitcoin-rpc-url bitcoind:18443 wallet --server-url http://ord:1338 receive | jq -r ".address")
docker exec -it bitcoind bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 101 $TO_ADDRESS
echo $TO_ADDRESS
