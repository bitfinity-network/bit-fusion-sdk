# Bridge Deployer

Bridge Deployer is a CLI tool for deploying and managing various types of bridge contracts on the Internet Computer and Ethereum networks.

## Requirements

- Rust
- Node.js
- Yarn

## Installation

Clone the repository and build the project:

git clone <https://github.com/your-repo/bridge-deployer.git>
cd bridge-deployer
cargo build --release

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
- `--ic-host <IC_HOST>`: IC host URL (default: <http://localhost:8080>)
- `--private-key <PRIVATE_KEY>`: Private Key of the wallet to use for the transaction
- `--evm-network <EVM_NETWORK>`: EVM network to deploy the contract to (e.g. "mainnet", "testnet", "local")
- `--deploy-bft`: Deploy the BFT bridge (default: false)
- `-v, --verbosity`: Set the verbosity level (use multiple times for higher levels)
- `-q, --quiet`: Silence all output

## Bridge Types

- Rune
- ICRC
- ERC20
- BTC

## Deployment Examples

### Deploying a Rune Bridge

```bash
./bridge-deployer deploy rune \
  --identity path/to/identity.pem \
  --private-key <PRIVATE_KEY> \
  --ic-host <http://localhost:8080> \
  --evm-network local \
  --config.admin <ADMIN_PRINCIPAL> \
  --config.min-confirmations 6 \
  --config.no-of-indexers 3 \
  --config.indexer-urls <https://indexer1.com,https://indexer2.com,https://indexer3.com> \
  --config.deposit-fee 1000000 \
  --config.mempool-timeout 3600
```

### Deploying an ICRC Bridge

```bash
./bridge-deployer deploy icrc \
  --identity path/to/identity.pem \
  --private-key <PRIVATE_KEY> \
  --ic-host <http://localhost:8080> \
  --evm-network local \
  --config.owner <OWNER_PRINCIPAL>
```

### Deploying an ERC20 Bridge

```bash
./bridge-deployer deploy erc20 \
  --identity path/to/identity.pem \
  --private-key <PRIVATE_KEY> \
  --ic-host <http://localhost:8080> \
  --evm-network local \
  --init.owner <OWNER_PRINCIPAL> \
  --erc.evm-link <EVM_LINK_PRINCIPAL>
```

### Upgrading a Bridge

```bash
./bridge-deployer upgrade \
  --identity path/to/identity.pem \
  --ic-host <http://localhost:8080> \
  --canister-id <CANISTER_ID>
```

### Deploying BFT Contract

To deploy the BFT contract alongside a bridge, add the `--deploy-bft` flag and provide the necessary BFT arguments:

```bash
./bridge-deployer deploy <BRIDGE_TYPE> \
  --identity path/to/identity.pem \
  --private-key <PRIVATE_KEY> \
  --ic-host <http://localhost:8080> \
  --evm-network local \
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
