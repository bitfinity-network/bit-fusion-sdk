#!/usr/bin/env bash

set -e
set -x

cd solidity

echo "soldeer install deps..."
forge soldeer install

echo "forge test..."
forge test -vv

echo "forge build..."
forge build --force --sizes

cd ..
