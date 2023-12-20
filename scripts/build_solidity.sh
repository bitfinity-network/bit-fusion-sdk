#!/usr/bin/env bash

set -e
set -x

cd solidity

echo "forge install..."
forge install

echo "forge test..."
forge test

echo "forge build..."
forge build --force

cd ..