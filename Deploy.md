# How to deploy ICRC-2 Bridge

- Build the canisters:

```bash
just build_all_canisters
```

- Build the solidity contracts:

```bash
just build_solidity
```

- Build the bridge deployer tool:

```bash
just build_bridge_deployer_tool
```

- For the testnet/mainnet, you need to have a ethereum private key with enough balance to deploy the BFT Bridge contract, and enough ICP to pay for the canisters deployment.

- Export the following environment variables:

```bash
export PRIVATE_KEY=<PRIVATE_KEY> (already done won't show this)
```

- Replace the <PRIVATE_KEY> with your private key

Important note: The private key must be in hex format, not in base64.

## Deployment to the mainnet should be done with the following command

- Note: You should change the values of the parameters according to your needs.
- Deploying this to mainnet

```bash
cargo run -p bridge-deployer -- --deploy-bft -vvvv --evm-network mainnet --identity ~/.config/dfx/identity/yasir/identity.pem  -i "https://ic0.app" deploy --wasm .artifact/icrc2-bridge.wasm --wallet-canister uc26k-4iaaa-aaaal-qdpga-cai icrc --evm-link i3jjb-wqaaa-aaaaa-qadrq-cai --signing-key-id production --owner xxwao-vj5ju-bpif6-mx6w4-w7zzu-g5s3o-p7ysu-sfnzb-ftjoo-jmer7-dqe --log-filter info
```

Deployment Results:
Canister ID for ICRC-1: yp5i2-haaaa-aaaal-qmzma-cai
WrappedTokenDeployer address: 0x72f46cCAdAC553b46C9E119ec173363e2E42B342
BFT deployed to: 0x6a98353c66Fdd4b6D76FfB0E8AF2a0d054952878
Implementation deployed to: 0xC904C413729DAe88B4f56D726A6F9878689F3829
Fee charge address: 0xb9bBaA9975Ab98259a6340329E66E17B3440b6aA

# SUCCESS

# One last step remaining (Setting the BFT Bridge)

dfx canister call --network ic yp5i2-haaaa-aaaal-qmzma-cai set_bft_bridge_contract "(\"0x6a98353c66Fdd4b6D76FfB0E8AF2a0d054952878\")" --candid=./.artifact/icrc2-bridge.did

DONE!!!!!!
