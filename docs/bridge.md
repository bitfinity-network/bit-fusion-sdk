# Deploying and Upgrading BFTBridge Contract

This README provides instructions on how to deploy and upgrade the BFTBridge contract using the OpenZeppelin [UUPS (Universal Upgradeable Proxy Standard)](https://docs.openzeppelin.com/upgrades-plugins/1.x/) proxy pattern and [Foundry](https://book.getfoundry.sh/) development framework.

## Requirements

Before proceeding with the deployment and upgrade process, ensure that you have the following:

- [Foundry](https://book.getfoundry.sh/getting-started/installation) installed and set up in your development environment.
- The necessary environment variables set:
  - `BRIDGE_ADDRESS`: The address of the minter contract.
  - `FEE_CHARGE_ADDRESS`: The address where fees will be charged.
  - `IS_WRAPPED_SIDE`: A boolean indicating whether the contract is on the wrapped side.
  - `PROXY_ADDRESS` (for upgrades): The address of the existing proxy contract.

## Deployment

To deploy the BFTBridge contract using the UUPS proxy pattern, follow these steps:

1. Open the [DeployBft.s.sol](solidity/script/DeployBft.s.sol) script file located in the `solidity/script` directory.

2. Review the script and ensure that the required environment variables are set correctly.

3. Run the deployment script using Foundry:

   ```bash
   forge script script/DeployBft.s.sol --rpc-url <your-rpc-url> --private-key <your-private-key> --broadcast \
         --skip-simulation \
         --evm-version paris \
         --optimize
   ```

   Replace `<your-rpc-url>` with the URL of your Ethereum RPC endpoint and `<your-private-key>` with your private key.

4. The script will deploy the BFTBridge contract using the UUPS proxy pattern and initialize it with the provided parameters.

5. After the deployment is complete, the script will output the addresses of the deployed proxy contract and the implementation contract.

Note: You can also use the bash script located [here]("../scripts/BftBridge/deploy.sh) to deploy the contract.

## Upgrade

To upgrade the BFTBridge contract to a new version, follow these steps:

1. Open the [UpgradeBft.s.sol](solidity/script/UpgradeBft.s.sol) script file located in the `solidity/script` directory.

2. Review the script and ensure that the required environment variables are set correctly, including the `PROXY_ADDRESS` variable, which should point to the address of the existing proxy contract.

3. Update the `newImplementation` variable in the script to specify the new contract version. For example:

   string memory newImplementation = "BftBridge.sol:BFTBridgeV2";

4. Run the upgrade script using Foundry:

   ```bash
   forge script script/UpgradeBft.s.sol --rpc-url <your-rpc-url> --private-key <your-private-key> --broadcast \
         --skip-simulation \
         --evm-version paris \
         --optimize
   ```

   Replace `<your-rpc-url>` with the URL of your Ethereum RPC endpoint and `<your-private-key>` with your private key.

5. The script will upgrade the existing proxy contract to the new implementation version and initialize it with the provided parameters.

6. After the upgrade is complete, the script will output the address of the upgraded proxy contract and the new implementation contract.

## Important Considerations

When upgrading contracts using the UUPS proxy pattern, keep the following in mind:

- **Storage Layout Collisions**: Ensure that the storage layout of the new implementation contract is compatible with the previous version. Adding, removing, or changing the order of state variables can lead to storage collisions and unexpected behavior.

- **Initialization**: If the new implementation contract requires additional initialization, make sure to update the `initializeData` variable in the upgrade script accordingly.

- **Annotation**: When upgrading to a new implementation contract,you MUST define a reference contract for storage layout comparisons. Otherwise, you will not receive errors if there are any storage layout incompatibilities.  it is recommended to add the `@custom:oz-upgrades-from` annotation to the new contract, specifying the previous implementation contract as the reference. This annotation helps in tracking the upgrade history and ensures compatibility. Read more [Here](https://docs.openzeppelin.com/upgrades-plugins/1.x/api-core#define-reference-contracts)

  /\*\*

  - @custom:oz-upgrades-from BftBridge.sol:BFTBridge
    \*/
    contract BFTBridgeV2 {
    // ...
    }
