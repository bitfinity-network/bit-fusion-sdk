#!/usr/bin/env sh

# Mint BRC20 tokens to `WALLET_ADDRESS`.
# `WALLET_ADDRESS` env is required.
# Related JSON data with token mint params(for inscription) is stored in `ord-test-infra/brc20_mint.json`.

ord --regtest --bitcoin-rpc-user user --bitcoin-rpc-pass pass wallet inscribe --fee-rate 1 --file ord-test-infra/brc20_json_inscriptions/brc20_mint.json --destination $WALLET_ADDRESS
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest generatetoaddress 1 $WALLET_ADDRESS
bitcoin-cli -rpcuser=user -rpcpassword=pass -regtest getblockcount
