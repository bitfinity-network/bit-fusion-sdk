#!/bin/bash
# shellcheck disable=all
source .env

forge script --target-contract DeployBFTBridge --broadcast -v script/DeployBFT.s.sol --rpc-url https://testnet.bitfinity.network --private-key 0x80df97675d3186b3e2b8935b314b3ebdca53663b128832cc05cb86bd83b43471 --sender 0xfB0D14c07DA958bBB257346F49b2E9C9382c4888
