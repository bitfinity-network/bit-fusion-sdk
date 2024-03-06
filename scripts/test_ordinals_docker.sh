#!/usr/bin/env sh

# Init wallet
ord --regtest --rpc-url http://bitcoind:18443 --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet create
WALLET_ADDRESS=$(ord --regtest --rpc-url http://bitcoind:18443 --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet receive | jq -r ".address")
bitcoin-cli -rpcuser=user -rpcconnect=http://bitcoind -rpcpassword=pass -regtest generatetoaddress 101 $WALLET_ADDRESS

# Deploy brc20
ord --regtest --rpc-url http://bitcoind:18443 --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet inscribe --fee-rate 1 --file brc20_json_artifacts/brc20_deploy.json
bitcoin-cli -rpcuser=user -rpcconnect=http://bitcoind -rpcpassword=pass -regtest generatetoaddress 1 $WALLET_ADDRESS
