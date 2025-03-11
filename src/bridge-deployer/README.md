# Bridge Deployer

Bridge Deployer is a CLI tool for deploying and managing various types of bridge contracts on the Internet Computer and Ethereum networks.

## Glossary

- **EVM Canister**: canister installed on Internet Computer running the **Bitfinity EVM**
- **Principal**: A unique identifier for a canister on the Internet Computer
- **Base token**: the native token to bridge (e.g. USDT on Ethereum; BTC on Bitcoin, ...).
- **BTF Bridge**: An ethereum smart contract that bridges the Base token with the Wrapped token on the EVM side.
- **Wrapped token**: the smart contract of the token on the bridged side (e.g. USDT on Bitfinity EVM; BTC on Bitfinity EVM, ...). This token is either installed on the **Bitfinity EVM** or on the specified EVM network.
- **Bridge Network**: the Internet Computer network where the bridge canister will be deployed (e.g. ic, localhost).
- **EVM RPC**: the RPC URL of the EVM network which will the Wrapped token will be installed to.

## Requirements

- Rust
- Forge CLI Installed
- dfx (only for **local** deployment)
- Either the EVM canister or an EVM RPC URL pointing to a network where the Wrapped token is deployed.

## Installation

Build the project using the following commands:

```bash
cargo build -p bridge-deployer --release
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
- `--bridge-network <IC_NETWORK>`: Internet Computer network to deploy the bridge canister to (possible values: `ic` | `localhost`; default: localhost)
- `--evm-principal <PRINCIPAL>`: Principal of the EVM canister to configure the bridge to communicate with the provided EVM canister. (must be used if `evm-rpc` is not provided). This is the EVM canister where the BTF bridge and the wrapped token contracts are deployed. Ensure that your wallet has enough native tokens to deploy the contracts.
- `--evm-rpc <RPC_URL>`: EVM RPC URL to configure the bridge canister to communicate with a specific EVM network (must be used if `evm-principal` is not provided). The EVM RPC endpoint should be a valid HTTP URL and must be linked to an EVM where the BTF bridge and the wrapped token contract are deployed. Be aware that this operation will spend tokens (e.g. ETH) from your wallet by deploying the contracts.
- `--canister-ids <PATH_TO_CANISTER_IDS>`: Path to the file containing the canister ids
- `-v, --verbosity`: Set the verbosity level (use multiple times for higher levels)
- `-q, --quiet`: Silence all output

## Bridge Types

- Rune
- ICRC
- ERC20
- BTC
- BRC20

## Deployment

For the deployment of canisters, you will require to have/create a wallet canister which will be used to deploy the bridge canister. The wallet canister should have enough ICPs to cover the deployment cost and the cycles required for the bridge canister to be operational.

For the deployment, you will need to provide the wallet canister id as `--wallet-canister` or as an environment variable `WALLET_CANISTER`.

Command to deploy a bridge canister connecting the bridge to the EVM canister:

```bash
./bridge-deployer
  -vvv \
  --bridge-network localhost \
  --private-key <PRIVATE_KEY> \
  --identity path/to/identity.pem \
  --evm-canister <EVM_PRINCIPAL|mainnet|testnet> \
  deploy \
  --wasm path/to/rune_bridge.wasm \
  --wallet-canister <WALLET_CANISTER> \
  rune \
  --owner <ADMIN_PRINCIPAL> \
  --min-confirmations 6 \
  --indexer-urls <https://indexer1.com,https://indexer2.com,https://indexer3.com> \
  --deposit-fee 1000000 \
  --mempool-timeout 3600 \
  --signing-key-id dfx \
  --bitcoin-network <bitcoin_network> \
  --indexer-consensus-threshold 3
```

If you want to connect the bridge to a custom EVM node with RPC you can use the `--evm-rpc` argument in place of `--evm-canister`.

For more detailed information on each command and its options, use the `--help` flag:

```bash
./bridge-deployer --help
```

## Upgrading a Bridge

To upgrade a bridge, you will need to provide the canister id of the bridge to be upgraded. The command is similar to the commands shown above, with the addition of the `--canister-id` argument.

```bash
./bridge-deployer upgrade [BRIDGE_TYPE] --wasm <WASM_PATH> --canister-id <CANISTER_ID>
```

## Reinstalling a Bridge

To reinstall a bridge, you will need to provide the canister id of the bridge to be reinstalled. The command is similar to the deployment command, with the addition of the `--canister-id` argument.

```bash
bridge-deployer reinstall [BRIDGE_TYPE] --canister-id <PRINCIPAL> --wasm <WASM_PATH> --btf-bridge <ADDRESS>
```

Note: You need to provide the canister arguments for the bridge type you are reinstalling.

### Bridge-Specific Deployment Examples

#### ICRC Bridge

```bash
bridge-deployer -vvv \
  --bridge-network mainnet \
  --identity <IDENTITY_PATH> \
  deploy \
  --wasm ./icrc_bridge.wasm \
  --wallet-canister rrkah-fqaaa-aaaaa-aaaaq-cai \
  icrc \
  --signing-key-id production \
  --owner 2vxsx-fae \
  --evm-principal <EVM_PRINCIPAL> \
  --log-filter "trace" # You can set the log filter to "trace", "debug", "info", "warn", "error"
```

The other bridges are more or less similar to the ICRC bridge, with the only difference being the arguments required for the specific bridge type. You can refer to the help text for each bridge type for more information.

Note: The examples above are for illustrative purposes only. Please replace the placeholders with the actual values.

### Extra Information

For the upgrade process, only the canister will be upgraded. The BTF contract will remain the same, hence the BTF contract should be deployed separately in case it requires an upgrade.

### Troubleshooting

- If you encounter any issues during the deployment process, double check all the arguments provided and ensure that the wallet canister has enough ICPs to cover the deployment cost as well as the Ethereum address.
- If the deployment fails, check the error message for more information on the cause of the failure.
