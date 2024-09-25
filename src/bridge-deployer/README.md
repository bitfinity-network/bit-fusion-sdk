# Bridge Deployer

Bridge Deployer is a CLI tool for deploying and managing various types of bridge contracts on the Internet Computer and Ethereum networks.

## Requirements

- Rust
- Node.js
- Yarn

## Installation

Clone the repository and build the project:

```bash
git clone <https://github.com/your-repo/bridge-deployer.git>
cd bridge-deployer
cargo build --release
```

## Usage

The general syntax for using the Bridge Deployer is:

```bash
./bridge-deployer [OPTIONS] <COMMAND>
```

## Commands

- `deploy`: Deploy a new bridge
- `upgrade`: Upgrade an existing bridge
- `reinstall`: Reinstall a bridge

## Global Options

- `--identity <IDENTITY_PATH>`: Path to the identity file
- `--private-key <PRIVATE_KEY>`: Private Key of the wallet to use for the transaction (can be provided as an environment variable `PRIVATE_KEY`)
- `--evm-network <EVM_NETWORK>`: EVM network to deploy the contract to (e.g. "mainnet", "testnet", "localhost")
- `--deploy-bft`: Deploy the BFT bridge (default: false)
- `-v, --verbosity`: Set the verbosity level (use multiple times for higher levels)
- `-q, --quiet`: Silence all output

## Bridge Types

- Rune
- ICRC
- ERC20
- BTC

## Deployment

For the deployment of canisters, you will require to have/create a wallet canister which will be used to deploy the bridge canister. The wallet canister should have enough ICPs to cover the deployment cost and the cycles required for the bridge canister to be operational.

For the deployment, you will need to provide the wallet canister id as `--wallet-canister` or as an environment variable `WALLET_CANISTER`.

### Deploying a Rune Bridge

```bash
./bridge-deployer
  -vvv \
  deploy \
  --wasm path/to/rune_bridge.wasm \
    rune \
  --identity path/to/identity.pem \
  --private-key <PRIVATE_KEY> \
  --evm-network localhost \
  --admin <ADMIN_PRINCIPAL> \
  --min-confirmations 6 \
  --no-of-indexers 3 \
  --indexer-urls <https://indexer1.com,https://indexer2.com,https://indexer3.com> \
  --deposit-fee 1000000 \
  --mempool-timeout 3600 \
  --signing-key-id dfx \
  --wallet-canister <WALLET_CANISTER> \
  --evm-principal <EVM_PRINCIPAL>
```

For the other bridge types, the command is similar to the one above, with the necessary arguments for the specific bridge type. Check the help command for more information on the arguments required for each bridge type.

```bash
./bridge-deployer deploy <BRIDGE_TYPE> --help
```

### Reinstalling a Bridge

To reinstall a bridge, you will need to provide the canister id of the bridge to be reinstalled. The command is similar to the deployment command, with the addition of the `--canister-id` argument.

```bash
./bridge-deployer
  -vvv \
  reinstall \
  --wasm path/to/rune_bridge.wasm \
    rune \
  --identity path/to/identity.pem \
  --private-key <PRIVATE_KEY> \
  --evm-network localhost \
  --admin <ADMIN_PRINCIPAL> \
  --min-confirmations 6 \
  --no-of-indexers 3 \
  --indexer-urls <https://indexer1.com,https://indexer2.com,https://indexer3.com> \
  --deposit-fee 1000000 \
  --mempool-timeout 3600 \
  --signing-key-id dfx \
  --evm-principal <EVM_PRINCIPAL> \
  --canister-id <CANISTER_ID>
```

### Upgrading a Bridge

To upgrade a bridge, you will need to provide the canister id of the bridge to be upgraded. The command is similar to the commands shown above, with the addition of the `--canister-id` argument.

Note: The BFT contract will not be upgraded during the upgrade process. The BFT contract should be deployed separately in case it requires an upgrade.

```bash
./bridge-deployer -vv upgrade --help
```

### Deploying BFT Contract

To deploy the BFT contract alongside a bridge, add the `--deploy-bft` flag, by default the flag is set to false.

The arguments `controller` and `owner` are optional. If not provided, the BFT contract will be deployed with the default controller(sender) and owner(sender) address.

Before deploying the BFT contract, make sure you have some funds in the wallet address to cover the deployment gas fees.

```bash
./bridge-deployer deploy <BRIDGE_TYPE> \
  --identity path/to/identity.pem \
  --private-key <PRIVATE_KEY> \
  --evm-network localhost \
  --deploy-bft \
  --owner <BFT_OWNER_ADDRESS> \
  --controllers <BFT_CONTROLLER_ADDRESSES>
```

For more detailed information on each command and its options, use the --help flag:

```bash
./bridge-deployer --help
./bridge-deployer <COMMAND> --help
```

Note: The examples above are for illustrative purposes only. Please replace the placeholders with the actual values.

### Extra Information

For the upgrade process, only the canister will be upgraded. The BFT contract will remain the same, hence the BFT contract should be deployed separately in case it requires an upgrade.

### Troubleshooting

- If you encounter any issues during the deployment process, double check all the arguments provided and ensure that the wallet canister has enough ICPs to cover the deployment cost as well as the Ethereum address.
- If the deployment fails, check the error message for more information on the cause of the failure.
