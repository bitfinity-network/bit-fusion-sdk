# Uniswap Contracts

This repository is part of the Uniswap project for the EVMC. It contains several smart contracts as well as deployment scripts.
By using the [Hardhat](https://hardhat.org/) project setup, all necessary contracts can be deployed to
the EVMC.
Hardhat is currently configured for deployment on two chains, either your localhost or the mentioned Bitfinity chain.

## Setup

In the root folder, create a `.env` file with the following values:

``` bash
UNISWAP_DEPLOYER="<private key of a valid EVMC account to deploy contracts>"
UNISWAP_DEPLOYER_2 ="<second and different private key of another valid EVMC account to deploy contracts>"
```

The first PK for the network is used in the scripts to deploy the tokens and faucet contract, whereas the secondary PK  or account is used to test and try the functionality of the deployed contracts.

After a successful setup, you start deploying the contracts yourself.

## Quickstart

- Install dependencies: `yarn`
- Deploy all contracts and example tokens: `yarn deploy:[evmc|localhost|evmcMain]`

---

## Deployment  - Uniswap Contracts

To deploy all smart contracts related to Uniswap functionalities, you can run the following command:

``` bash
hardhat run scripts/deploy-uniswap.ts --network evmc
```

---

## Deployment - Faucet and ERC20 Tokens

To deploy the faucet and all custom Tokens, you can run the following command:

``` bash
hardhat run scripts/deploy-tokens.ts --network evmc
```

The script will deploy the `Faucet.sol` smart contract that serves as a faucet that can distribute coins to other
addresses. This enables users in the front end to claim some of our distributed tokens on the Bitfinity Ethereum network. Moreover, the script will deploy all the ERC20 tokens from `EvmcTokens.sol`, and the `Faucet.sol` will be approved to spend the tokens.

---

## Logging

The addresses of the various deployed contracts will be locally logged and saved into `.json` files inside the `./logs`  directory on your machine in case you want to reinspect some of the details of your deployments later.

To differentiate which network the contracts were deployed to, the file names are prefixed with the network chain ID. For instance, addresses for the EVMC network (chainID = 355113) can be found locally after script execution in `./logs/tokenAddresses.json`.
