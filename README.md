# ckOrd

This repo contains canisters for executing Bitcoin Ordinal inscriptions.

## Quick Start

1. Please make sure the Docker engine is running, and then run the following command to start Bitcoin and Ord:

```bash
cd ckOrd
./scripts/init.sh
```

2. In a separate terminal, start the `dfx` replica:

```bash
dfx start --clean --bitcoin-node 127.0.0.1:18443
```

3. Build and deploy the canister

```bash
./scripts/build.sh
./scripts/deploy.sh init
```

4. Interact with the canister (TODO)
