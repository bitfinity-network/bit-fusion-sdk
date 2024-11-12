# BTF Bridge Forge Scripts

This comprehensive guide provides detailed instructions for running various Forge scripts within the project. These scripts are essential for deploying, upgrading, and managing the BTF Bridge and related contracts.

## Table of Contents

1. [DeployBTF.s.sol](#deploybtfssol)
2. [UpgradeBTF.s.sol](#upgradebtfssol)
3. [PauseUnpause.s.sol](#pauseunpausessol)
4. [DeployFeeCharge.s.sol](#deployfeechargessol)
5. [DeployWrappedToken.s.sol](#deploywrappedtokenssol)
6. [DeployWrappedTokenDeployer.s.sol](#deploywrappedtokendeployerssol)

## Prerequisites

Before running any scripts, ensure you have the following:

- Forge CLI installed
- Access to an Ethereum RPC endpoint
- A funded account with the necessary permissions

## DeployBTF.s.sol

This script is responsible for deploying the BTFBridge contract, a crucial component of the BTF Bridge system.

### Usage

Execute the following command to deploy the BTFBridge:

forge script script/DeployBTF.s.sol:DeployBTFBridge --rpc-url <your_rpc_url> --broadcast --private-key <your_private_key> --sender <your_sender_address>

### Environment Variables

To customize the deployment, set the following environment variables:

- `PRIVATE_KEY`: The private key of the deployer account
- `MINTER_ADDRESS`: Address of the designated minter
- `FEE_CHARGE_ADDRESS`: Address of the fee charge contract
- `WRAPPED_TOKEN_DEPLOYER`: Address of the wrapped token deployer
- `IS_WRAPPED_SIDE`: Boolean flag indicating if this is the wrapped side (true/false)
- `OWNER`: (Optional) Address of the contract owner
- `CONTROLLERS`: (Optional) Comma-separated list of controller addresses

## UpgradeBTF.s.sol

This script facilitates the upgrade process for the BTFBridge contract. It consists of three separate contracts, each handling a specific step in the upgrade process.

#### Usage

Execute the following commands in sequence to perform the upgrade:

1. Deploy the new implementation:

   forge script script/UpgradeBTF.s.sol:PrepareUpgrade --rpc-url <your_rpc_url> --broadcast --private-key <your_private_key> --sender <your_sender_address>

2. Add the new implementation to the proxy's allowed implementations:

   forge script script/UpgradeBTF.s.sol:AddNewImplementation --rpc-url <your_rpc_url> --broadcast --private-key <your_private_key> --sender <your_sender_address>

3. Upgrade the proxy to use the new implementation:

   forge script script/UpgradeBTF.s.sol:UpgradeProxy --rpc-url <your_rpc_url> --broadcast --private-key <your_private_key> --sender <your_sender_address>

### Environment Variables

Set the following environment variables for the upgrade process:

- `PROXY_ADDRESS`: Address of the proxy contract
- `NEW_IMPLEMENTATION_ADDRESS`: (For UpgradeProxy) Address of the new implementation contract

## PauseUnpause.s.sol

This script allows for the pausing and unpausing of the BTFBridge contract, providing an essential safety mechanism.

### Usage

To pause or unpause the contract, use the following command:

forge script script/PauseUnpause.s.sol:PauseUnpauseScript --rpc-url <your_rpc_url> --broadcast --sig "run(address,bool)" <contract_address> <true_to_pause_false_to_unpause> --private-key <your_private_key> --sender <your_sender_address>

### Environment Variables

- `PRIVATE_KEY`: The private key of the owner account with pause/unpause permissions

## DeployFeeCharge.s.sol

This script deploys the FeeCharge contract, which handles fee collection for the BTF Bridge system.

### Usage

Deploy the FeeCharge contract with the following command:

forge script script/DeployFeeCharge.s.sol:DeployFeeCharge --rpc-url <your_rpc_url> --broadcast --private-key <your_private_key> --sender <your_sender_address>

### Environment Variables

Configure the deployment with these environment variables:

- `BRIDGES`: Comma-separated list of bridge addresses
- `EXPECTED_ADDRESS`: (Optional) Expected address of the deployed contract for verification

## DeployWrappedToken.s.sol

This script deploys a new wrapped token using the WrappedTokenDeployer contract.

### Usage

To deploy a new wrapped token, execute:

forge script script/DeployWrappedToken.s.sol:DeployWrappedToken --rpc-url <your_rpc_url> --broadcast --private-key <your_private_key> --sender <your_sender_address>

### Environment Variables

Set these environment variables to customize the wrapped token:

- `WRAPPED_TOKEN_DEPLOYER`: Address of the WrappedTokenDeployer contract
- `NAME`: Name of the new wrapped token
- `SYMBOL`: Symbol of the new wrapped token
- `DECIMALS`: Number of decimals for the new wrapped token
- `OWNER`: Address of the token owner

## DeployWrappedTokenDeployer.s.sol

This script deploys the WrappedTokenDeployer contract, which is used to create new wrapped tokens.

### Usage

Deploy the WrappedTokenDeployer with this command:

forge script script/DeployWrappedTokenDeployer.s.sol:DeployWrappedTokenDeployer --rpc-url <your_rpc_url> --broadcast --private-key <your_private_key> --sender <your_sender_address>

No specific environment variables are required for this script.

---
NOTE: By following these instructions, you can effectively manage and deploy various components of the BTF Bridge system using Forge scripts. Always ensure you're using the correct RPC URL, private key, and sender address when executing these scripts.
