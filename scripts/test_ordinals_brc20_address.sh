#!/usr/bin/env sh

# Receive temp brc20 address
TO_ADDRESS=$(ord --regtest --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet receive | jq -r ".address")
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 101 $TO_ADDRESS
echo $TO_ADDRESS
