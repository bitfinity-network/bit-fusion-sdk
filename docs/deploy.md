# Deploy

- [Deploy](#deploy)
    - [Requirements](#requirements)
        - [Ubuntu 24.04 additional dependencies](#ubuntu-2404-additional-dependencies)
    - [Build](#build)
    - [Local test EVM deployment](#local-test-evm-deployment)
    - [Bridge deployment](#bridge-deployment)
        - [Erc20 bridge](#erc20-bridge)

## Requirements

- [Get `dfx` here](https://internetcomputer.org/docs/current/developer-docs/getting-started/install/#installing-dfx) if
  you don't have it already.
- [Install the Rust toolchain](https://www.rust-lang.org/tools/install) if it's not already installed.
- Install the rust wasm32 target: `rustup target add wasm32-unknown-unknown`
- [Download and install Docker, with Compose](https://www.docker.com/products/docker-desktop/) if you don't already have
  it.
- Install [foundry](https://book.getfoundry.sh/getting-started/installation).

    ```sh
    curl -L https://foundry.paradigm.xyz | bash
    ```

- Install [just](https://just.systems/) command runner.
- Install SSL certificates to make the `local-ssl-proxy` in the docker images able to work.

  On Debian-based systems it should be enough to run

    ```sh
    sudo cp btc-deploy/mkcert/* /usr/local/share/ca-certificates/
    sudo update-ca-certificates --fresh --verbose
    ```

  On Arch linux based systems

    ```sh
    sudo trust anchor btc-deploy/mkcert/*
    sudo update-ca-trust
    ```

  While on MacOS you should install them by clicking on the certificates in the mkcert folder.

### Ubuntu 24.04 additional dependencies

- Install libunwind: `sudo apt install libunwind-dev`
- Install protobuf: `sudo apt install protobuf-compiler`

## Build

Use `just` command to build canisters and contracts:

```shell
just build_solidity
just build_all_canisters
```

## Local test EVM deployment

Deploy local EVM with this command:

```shell
just deploy_evm
```

After the EVM is deployed you need to mint some native tokens to you wallet address:

```shell
dfx canister call evm admin_mint_native_tokens '("<WALLET HEX ADDRESS>", "10000000000000000000")'
```

## Bridge deployment

To simplify deployment and configuring of the bridge canisters a CLI tool `bridge-deployer` is provided. Some examples
about how to use this tool are given below. You can get list of all available options for each command by running:

```shell
cargo run -p bridge-deployer -- --help
```

All commands of bridge deployer require a private key of you EVM wallet to run EVM operations. It can be specified:

* through `.env` file with content `PRIVATE_KEY=<KEY>`
* through setting `PRIVATE_KEY` environment variable
* through command line argument `--private-key <KEY>`

### Erc20 bridge

Deploy bridge canister and BTF bridge contract:

```shell
cargo run -p bridge-deployer -- deploy --wasm .artifact/erc20-bridge.wasm.gz \
  erc20 --base-evm-url https://testnet.bitfinity.network
```

After the bridge was deployed, create a wrapped token (using addresses from the previous command as inputs):

```shell
cargo run -p bridge-deployer -- wrap erc20 \
  --base-evm-url https://testnet.bitfinity.network \
  --base-btf-address <BASE_BTF_CONTRACT_ADDRESS> \
  --wrapped-btf-address <WRAPPED_BTF_CONTRACT_ADDRESS> \
  --token-address <BASE_TOKEN_ADDRESS>
```
