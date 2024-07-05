# BitFusion SDK Hardhat Tasks

This project contains several Hardhat tasks for deploying and managing contracts in the BitFusion SDK.

## Setup

1. Install Node.js and yarn if you haven't already.
2. Clone this repository and navigate to the project directory.
3. Install dependencies:

    ```bash
    yarn install
    ```

4. Create a `.env` file in the root directory and add your environment variables:

    ```bash
    PRIVATE_KEY=your_private_key
    ```

5. Configure your [hardhat config](../solidity/hardhat.config.ts) file to use the appropriate network settings.

## Available Tasks

### 1. Deploy BFT Contract

Deploys the BFT contract.

```bash
npx hardhat deploy-bft --network <network> --minter-address <minter_address> --fee-charge-address <fee_charge_address> --is-wrapped-side <true|false>
```

### 2. Deploy Fee Charge Contract

Deploys the Fee Charge contract.

```bash
npx hardhat deploy-fee-charge --network <network> --bridges <bridge_addresses> [--nonce <nonce>] [--expected-address <expected_address>]
```

### 3. Compute Fee Charge Address

Computes the expected address of the Fee Charge contract.

```bash
npx hardhat fee-charge-address --network <network> --nonce <nonce> [--deployer-address <deployer_address>]
```

### 4. Pause Contract (Specific to BFT contract)

Pauses a deployed contract.

```bash
npx hardhat pause --network <network> --contract <contract_address>
```

### 5. Unpause Contract (Specific to BFT contract)

Unpauses a deployed contract.

```bash
npx hardhat unpause --network <network> --contract <contract_address>
```

### 6. Upgrade BFT Contract

Upgrades the BFT contract to a new implementation.

Before attempting to upgrade the contract, follow these steps:

- Create a new versioned file named `BFTBridgeVX.sol` in the same directory as your original `BFTBridge.sol` file.

- In your `BFTBridgeV2.sol` file, add new state variables, events, or functions as needed. Here's an example:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.7;

import "./BftBridge.sol";

/// @custom:oz-upgrades-from src/BftBridge.sol:BFTBridge
contract BFTBridgeV2 is BFTBridge {
    // Add new state variables here

    // Add new events here

    // Add new functions here

    // Override existing functions if needed

    // If you need to initialize new state variables, create a reinitializer function:

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    function reinitialize() public reinitializer(2) {
        // Initialize new state variables or update existing ones
    }
}
```

- Update your upgrade script found [here](../solidity/tasks/upgrade-bft.ts) and make necessary changes according to your initializer functions and contract versioning.

- Run the upgrade task:

```bash
npx hardhat upgrade-bft --network <network> --proxy-address <proxy_address> --extra-args-your-added-reflecting-your-new-contract
```

**Important Considerations:**

- Always test your upgrades on a testnet before applying them to mainnet.
- Monitor for any unexpected behavior after the upgrade.
- Ensure your new implementation is fully compatible with the existing storage layout.
- Be cautious with storage variable changes, as they can cause conflicts.

## Usage Examples

1. **Deploy BFT Contract:**

    npx hardhat deploy-bft --network mainnet --minter-address 0x1234... --fee-charge-address 0x5678... --is-wrapped-side true

2. **Deploy Fee Charge Contract:**

    npx hardhat deploy-fee-charge --network mainnet --bridges 0x1234...,0x5678... --nonce 5

3. **Compute Fee Charge Address:**

    npx hardhat fee-charge-address --network mainnet --nonce 10 --deployer-address 0x1234...

4. **Pause Contract:**

    npx hardhat pause --network mainnet --contract 0x1234...

5. **Unpause Contract:**

    npx hardhat unpause --network mainnet --contract 0x1234...

6. **Upgrade BFT Contract:**

    npx hardhat upgrade-bft --network mainnet --proxy-address 0x1234...

## Best Practices and Notes

- Always ensure you're connected to the correct network before executing tasks.
- Double-check all addresses and parameters before running tasks, especially for mainnet deployments.
- Some tasks may require specific roles or permissions on the contracts.
- Keep your private keys secure and never share them publicly.
- Use a separate account for testing and development purposes.

## Troubleshooting

If you encounter any issues:

1. Ensure all dependencies are correctly installed by running `yarn install` again.
2. Verify that your `.env` file is correctly set up with the required API keys and private keys.
3. Check that you're using the correct network in your Hardhat configuration.
4. Make sure you have sufficient funds in your account for gas fees when deploying or interacting with contracts.
5. Check the console output for any error messages and refer to the Hardhat documentation for specific error codes.
6. If you're experiencing network-related issues, try using a different RPC endpoint or waiting for a few minutes before retrying.

For more detailed information about each task, refer to the individual task files in the [`tasks`](../solidity/tasks) directory.

## Additional Resources

- [Hardhat Documentation](https://hardhat.org/getting-started/)
- [OpenZeppelin Upgrades](https://docs.openzeppelin.com/upgrades-plugins/1.x/)
- [Ethereum Development Best Practices](https://consensys.github.io/smart-contract-best-practices/)

If you need further assistance, don't hesitate to reach out to the BitFusion SDK support team or consult the community forums.
