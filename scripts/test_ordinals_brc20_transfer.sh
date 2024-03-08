#!/usr/bin/env sh

# Transfer brc20
IID=$(ord --regtest --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet inscribe --fee-rate 1 --file brc20_json_artifacts/brc20_transfer.json --destination $WALLET_ADDRESS | jq -r ".inscriptions[0].id")
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 1 $WALLET_ADDRESS
ord --regtest --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet send --fee-rate 1 $TO_ADDRESS $IID
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 1 $WALLET_ADDRESS
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest getblockcount
