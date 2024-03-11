#!/usr/bin/env sh

# Creates wallet and returns new address(other scripts require this address in `WALLET_ADDRESS` env, be sure to set with `export WALLET_ADDRESS=<ADDRESS>`).
# Also this script mine 101 blocks to activate wallet.

ord --regtest --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet create
WALLET_ADDRESS=$(ord --regtest --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet receive | jq -r ".address")
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 101 $WALLET_ADDRESS
echo $WALLET_ADDRESS
