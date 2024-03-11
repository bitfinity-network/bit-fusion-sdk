#!/usr/bin/env sh

# Generates sample address for receiving BRC20 tokens.
# Be sure to set `TO_ADDRESS` env after run this.

TO_ADDRESS=$(ord --regtest --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet receive | jq -r ".address")
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 101 $TO_ADDRESS
echo $TO_ADDRESS
