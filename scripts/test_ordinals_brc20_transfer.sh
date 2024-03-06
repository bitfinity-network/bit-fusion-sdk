#!/usr/bin/env sh

# Transfer brc20
ord --regtest --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet inscribe --fee-rate 1 --file brc20_json_artifacts/brc20_transfer.json
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 1 $WALLET_ADDRESS
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest getblockcount
