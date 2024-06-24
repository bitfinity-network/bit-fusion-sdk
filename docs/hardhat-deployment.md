
# BitFusion SDK Hardhat Tasks

This project contains several Hardhat tasks for deploying and managing contracts in the BitFusion SDK.

## Setup

1. Install Node.js and npm if you haven't already.
2. Clone this repository and navigate to the project directory.
3. Install dependencies:

   npm install

4. Create a `.env` file in the root directory and add your environment variables:

   PRIVATE_KEY=your_private_key

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

```bash
npx hardhat upgrade-bft --network <network> --proxy-address <proxy_address>
```

## Usage Examples

1. **Deploy BFT Contract:**

    ```bash
    npx hardhat deploy-bft --network mainnet --minter-address 0x1234... --fee-charge-address 0x5678... --is-wrapped-side true
    ```

2. **Deploy Fee Charge Contract:**

    ```bash
    npx hardhat deploy-fee-charge --network mainnet --bridges 0x1234...,0x5678... --nonce 5
    ```

3. **Compute Fee Charge Address:**

    ```bash
    npx hardhat fee-charge-address --network mainnet --nonce 10 --deployer-address 0x1234...
    ```

4. **Pause Contract:**

    ```bash
    npx hardhat pause --network mainnet --contract 0x1234...
    ```

5. **Unpause Contract:**

    ```bash
    npx hardhat unpause --network mainnet --contract 0x1234...
    ```

6. **Upgrade BFT Contract:**

    ```bash
    npx hardhat upgrade-bft --network mainnet --proxy-address 0x1234...
    ```

## Notes

- Always ensure you're connected to the correct network before executing tasks.
- Double-check all addresses and parameters before running tasks, especially for mainnet deployments.
- Some tasks may require specific roles or permissions on the contracts.

## Troubleshooting

If you encounter any issues:

1. Ensure all dependencies are correctly installed.
2. Verify that your `.env` file is correctly set up with the required API keys and private keys.
3. Check that you're using the correct network in your Hardhat configuration.
4. Make sure you have sufficient funds in your account for gas fees when deploying or interacting with contracts.

For more detailed information about each task, refer to the individual task files in the [`tasks`](../solidity/tasks) directory.
