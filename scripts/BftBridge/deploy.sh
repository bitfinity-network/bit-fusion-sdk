#!/bin/bash

# Function to display usage information
usage() {
    echo "Usage: $0 <rpc-url> <private-key> <minter-address> <fee-charge-address> <is-wrapped-side>"
    echo
    echo "Deploy the BFT Bridge smart contract using the provided parameters."
    echo
    echo "Arguments:"
    echo "  <rpc-url>            The RPC URL of the Ethereum network."
    echo "  <private-key>        The private key used to sign the deployment transaction."
    echo "  <minter-address>     The address of the minter account."
    echo "  <fee-charge-address> The address where fees will be charged."
    echo "  <is-wrapped-side>    Indicates whether this is the wrapped side of the bridge (true/false)."
    echo
    echo "Example:"
    echo "  $0 https://mainnet.bitfinity.network your-private-key 0x1234... 0x5678... true"
    exit 1
}

# Check if the required number of arguments is provided
if [ $# -ne 5 ]; then
    echo "Error: Invalid number of arguments."
    usage
fi

# Assign the provided arguments to variables
RPC_URL=$1
PRIVATE_KEY=$2
MINTER_ADDRESS=$3
FEE_CHARGE_ADDRESS=$4
IS_WRAPPED_SIDE=$5

# Validate the arguments
if [[ ! $RPC_URL =~ ^https?:// ]]; then
    echo "Error: Invalid RPC URL format."
    usage
fi

if [[ ! $PRIVATE_KEY =~ ^0x[a-fA-F0-9]{64}$ ]]; then
    echo "Error: Invalid private key format."
    usage
fi

if [[ ! $MINTER_ADDRESS =~ ^0x[a-fA-F0-9]{40}$ ]]; then
    echo "Error: Invalid minter address format."
    usage
fi

if [[ ! $FEE_CHARGE_ADDRESS =~ ^0x[a-fA-F0-9]{40}$ ]]; then
    echo "Error: Invalid fee charge address format."
    usage
fi

if [[ $IS_WRAPPED_SIDE != true && $IS_WRAPPED_SIDE != false ]]; then
    echo "Error: Invalid value for is-wrapped-side. Must be 'true' or 'false'."
    usage
fi

# Export the environment variables
export MINTER_ADDRESS="$MINTER_ADDRESS"
export FEE_CHARGE_ADDRESS="$FEE_CHARGE_ADDRESS"
export IS_WRAPPED_SIDE="$IS_WRAPPED_SIDE"

# Navigate to the directory containing the deployment script
cd solidity/script || {
    echo "Error: Directory 'solidity/script' not found."
    exit 1
}

# Run the deployment script using Foundry
forge script DeployBft.s.sol --rpc-url "$RPC_URL" --private-key "$PRIVATE_KEY" --broadcast \
    --skip-simulation \
    --evm-version paris \
    --optimize

# Check if the deployment was successful
if [ $? -eq 0 ]; then
    echo "Deployment successful."
else
    echo "Deployment failed."
    exit 1
fi
