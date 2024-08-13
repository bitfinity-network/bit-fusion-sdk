#!/usr/bin/env bash

set -e
set -x

cd solidity

echo "forge update and install deps..."
forge soldeer update

echo "forge test..."
forge test -vv

echo "forge build..."
forge build --force

cd ..
