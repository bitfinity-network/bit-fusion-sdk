# Bridge Deployer

Bridge Deployer is a CLI tool for deploying and managing various types of bridge contracts on the Internet Computer.

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

- `deploy`: Deploy a new bridge contract
- `upgrade`: Upgrade an existing bridge contract
- `reinstall`: Reinstall a bridge contract
- `list`: List all deployed contracts

## Global Options

- `--identity <IDENTITY_PATH>`: Path to the identity file
- `--ic-host <IC_HOST>`: IC host URL (default: <http://localhost:8080>)
- `--state-file <STATE_FILE>`: Path to the state file (default: canister_state.json)
- `-v, --verbosity`: Set the verbosity level (use multiple times for higher levels)

## Deployment Examples

### Deploying a Rune Bridge

```bash
./bridge-deployer deploy rune \
  --identity path/to/identity.pem \
  --wasm path/to/rune_bridge.wasm \
  --network mainnet \
  --evm-principal abcde-fghij-klmno-pqrst-uvwxy-z \
  --signing-key-id production \
  --admin principal_id_here \
  --min-confirmations 6 \
  --no-of-indexers 3 \
  --indexer-urls <https://indexer1.com,https://indexer2.com,https://indexer3.com> \
  --deposit-fee 1000000 \
  --mempool-timeout 3600
```

### Deploying an ICRC Bridge

```bash
./bridge-deployer deploy icrc \
  --identity path/to/identity.pem \
  --wasm path/to/icrc_bridge.wasm \
  --evm-principal abcde-fghij-klmno-pqrst-uvwxy-z \
  --signing-key-id production \
  --owner principal_id_here
```

### Deploying an ERC20 Bridge

```bash
./bridge-deployer deploy erc20 \
  --identity path/to/identity.pem \
  --wasm path/to/erc20_bridge.wasm \
  --evm-principal abcde-fghij-klmno-pqrst-uvwxy-z \
  --signing-key-id production \
  --owner principal_id_here \
  --evm-link principal_id_here
```

### Upgrading a Bridge

```bash
./bridge-deployer upgrade \
  --identity path/to/identity.pem \
  --canister-id abcde-fghij \
  --wasm path/to/new_bridge.wasm
```

### Listing Deployed Contracts

./bridge-deployer list

## State Management

The Bridge Deployer maintains a state file (default: canister_state.json) to keep track of deployed contracts and their deployment history. This file is automatically updated after each operation.

For more detailed information on each command and its options, use the --help flag:

```bash
./bridge-deployer --help
./bridge-deployer <COMMAND> --help
```
