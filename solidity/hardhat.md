# BitFusion SDK Hardhat Tasks and Contract Upgrade Guide

This document provides comprehensive information about the Hardhat tasks available in the BitFusion SDK, including the BFT contract upgrade process using OpenZeppelin's UUPS proxy pattern.

## Table of Contents

1. [Setup](#setup)
2. [Available Tasks](#available-tasks)
3. [BFT Contract Upgrade Process](#bft-contract-upgrade-process)
   - [OpenZeppelin UUPS Proxy Pattern](#openzeppelin-uups-proxy-pattern)
   - [Upgrade Methods](#upgrade-methods)
   - [Upgrade Considerations](#upgrade-considerations)
4. [Usage Examples](#usage-examples)
5. [Best Practices and Notes](#best-practices-and-notes)
6. [Troubleshooting](#troubleshooting)
7. [Additional Resources](#additional-resources)

## Setup

1. Install Node.js and yarn.
2. Clone the repository and navigate to the project directory.
3. Install dependencies:

   yarn install

4. Create a `.env` file in the root directory with your environment variables:

   PRIVATE_KEY=your_private_key

5. Configure your hardhat config file with appropriate network settings.

## Available Tasks

- Deploy BFT Contract:

```bash
npx hardhat deploy-bft --network <network> --minter-address <minter_address> --fee-charge-address <fee_charge_address> --is-wrapped-side <true|false>
```

- Deploy Fee Charge Contract:

```bash
npx hardhat deploy-fee-charge --network <network> --bridges <bridge_addresses> [--nonce <nonce>] [--expected-address <expected_address>]
```

- Compute Fee Charge Address:

```bash
npx hardhat fee-charge-address --network <network> --nonce <nonce> [--deployer-address <deployer_address>]
```

- Pause Contract:

```bash
npx hardhat pause --network <network> --contract <contract_address>
```

- Unpause Contract:

```bash
npx hardhat unpause --network <network> --contract <contract_address>
```

## BFT Contract Upgrade Process

### OpenZeppelin UUPS Proxy Pattern

The UUPS (Universal Upgradeable Proxy Standard) proxy pattern is an upgradeable contract system where:

- A proxy contract delegates all calls to an implementation contract.
- The implementation contract can be upgraded without changing the proxy's address.
- Upgrade logic is stored in the implementation, reducing proxy contract complexity.

Benefits:

- Gas efficiency
- Simplified proxy contract
- Consistent address for users

For more details, see the OpenZeppelin documentation on UUPS proxies.

### Upgrade Methods

#### Controlled Upgrade (Step-by-Step)

- Prepare Upgrade:

```bash
npx hardhat prepareUpgrade --network <network> --proxy-address <PROXY_ADDRESS> --updated-contract <NEW_CONTRACT_NAME>
```

- Add New Implementation:

```bash
npx hardhat addNewImplementation --network <network> --proxy-address <PROXY_ADDRESS> --reference-contract <REFERENCE_CONTRACT> --impl-address <NEW_IMPL_ADDRESS>
```

- Upgrade Proxy:

```bash
npx hardhat upgradeProxy --network <network> --proxy-address <PROXY_ADDRESS> --updated-contract-address <NEW_IMPL_ADDRESS> --updated-contract-name <NEW_CONTRACT_NAME> --reference-contract <REFERENCE_CONTRACT>
```

#### One-Go Upgrade

For a complete upgrade in one operation:

```bash
npx hardhat upgrade-bft-full --network <network> --proxy-address <PROXY_ADDRESS> --reference-contract <REFERENCE_CONTRACT> --updated-contract <NEW_CONTRACT_NAME>
```

### Upgrade Considerations

- The controlled upgrade offers better monitoring and control but requires more manual intervention.
- The one-go upgrade is convenient but comes with risks:
  - Atomicity: If any step fails, the entire upgrade fails.
  - Gas Costs: Combined operation may hit gas limits on some networks.
  - Reduced Control: Less opportunity to verify each step's success.
- Always thoroughly test upgrades on a testnet before applying to mainnet.

## Usage Examples

- Deploy BFT Contract:

```bash
npx hardhat deploy-bft --network mainnet --minter-address 0x1234... --fee-charge-address 0x5678... --is-wrapped-side true
```

- Deploy Fee Charge Contract:

```bash
npx hardhat deploy-fee-charge --network mainnet --bridges 0x1234...,0x5678... --nonce 5
```

- Compute Fee Charge Address:

```bash
npx hardhat fee-charge-address --network mainnet --nonce 10 --deployer-address 0x1234...
```

- Pause Contract:

```bash
npx hardhat pause --network mainnet --contract 0x1234...
```

- Unpause Contract:

```bash
npx hardhat unpause --network mainnet --contract 0x1234...
```

- Upgrade BFT Contract (One-Go):

```bash
npx hardhat upgrade-bft-full --network mainnet --proxy-address 0x1234... --reference-contract BFTBridge --updated-contract BFTBridgeV2
```

## Best Practices and Notes

- Always ensure you're connected to the correct network before executing tasks.
- Double-check all addresses and parameters before running tasks, especially for mainnet deployments.
- Some tasks may require specific roles or permissions on the contracts.
- Keep your private keys secure and never share them publicly.
- Use a separate account for testing and development purposes.

## Troubleshooting

If you encounter issues:

- Ensure all dependencies are correctly installed.
- Verify that your `.env` file is correctly set up.
- Check that you're using the correct network in your Hardhat configuration.
- Ensure sufficient funds for gas fees when deploying or interacting with contracts.
- Check console output for error messages and refer to Hardhat documentation for specific error codes.
- For network-related issues, try using a different RPC endpoint or wait before retrying.

## Additional Resources

- [Hardhat Documentation](https://hardhat.org/docs)
- [OpenZeppelin Upgrades](https://docs.openzeppelin.com/upgrades-plugins/1.x/)
- [Ethereum Development Best Practices](https://consensys.github.io/smart-contract-best-practices/)
