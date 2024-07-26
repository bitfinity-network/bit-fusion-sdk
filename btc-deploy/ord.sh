#!/usr/bin/env bash

echo "Ord version: $(ord --version)"

rm -rf /data/*
mkdir -p /data

local-ssl-proxy --source 8001 --target 8000 --key localhost+3-key.pem --cert localhost+3.crt &
ord -r --index-runes --bitcoin-rpc-url=http://bitcoind:18443 server --http-port=8000
