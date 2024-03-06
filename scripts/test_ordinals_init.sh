#!/usr/bin/env sh

# Init wallet
ord --regtest --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet create
WALLET_ADDRESS=$(ord --regtest --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet receive | jq -r ".address")
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 101 $WALLET_ADDRESS
echo $WALLET_ADDRESS
