# ckOrd

This repo contains the `Inscriber` canister for executing Bitcoin Ordinal inscriptions. To create an inscription, one needs to spend a `P2TR` (Pay-to-Taproot) transaction output. This usually involves two separate transactions: a `commit` and a `reveal`. The commit transaction involves spending one or more `P2PKH` (Pay-to-Public-Key-Hash) UTXOS, which are controlled by the `Inscriber` canister signed using ECDSA signatures. The result of this is a `P2TR` output that commits to a reveal script which contains the inscription.

The reveal transaction involves spending the `P2TR` output, which reveals the inscription by providing the reveal script and a Schnorr signature. This second transaction creates a new output associated with the destination address, which owns the inscription. If no destination address is provided, the inscription is sent to the canister's address.

The `Inscriber` (currently) provides support for two types of inscriptions: [BRC20](https://domo-2.gitbook.io/brc-20-experiment/) and `NFTs` (arbitrary inscriptions based on [Ordinal Theory](https://docs.ordinals.com/inscriptions.html)).

## Testing the Canister Locally

### Prerequisites

- Use `dfx` version <= `0.17.x` (for now). [Get `dfx` here](https://internetcomputer.org/docs/current/developer-docs/getting-started/install/#installing-dfx) if you don't have it already.
- [Install the Rust toolchain](https://www.rust-lang.org/tools/install) if it's not already installed.
- [Download and install Docker, with Compose](https://www.docker.com/products/docker-desktop/) if you don't already have it.

After installing Rust, add the `wasm32` target via:

```bash
rustup target add wasm32-unknown-unknown # Required for building IC canisters
```

## Step 1: Init, Build, and Deploy

### Clone this Repo

```bash
git clone https://github.com/bitfinity-network/ckOrd
cd ckOrd
```

### Init `bitcoind`

```bash
./scripts/init.sh
```

The above command will start `bitcoind` in a Docker container, create a wallet called "testwallet", and generate 101 blocks to make sure the wallet has enough bitcoins to spend. You might see an error in the logs if the "testwallet" already exists, but this is not a problem.

### Build the Canister

```bash
./scripts/build.sh
```

#### Start the Local IC Replica and Deploy

```bash
dfx start --clean --background --enable-bitcoin
./scripts/deploy.sh init
```

The above commands will start the local IC replica in the background, connecting it to the Bitcoin daemon in `regtest` mode, and then deploy the canister.

## Step 2: Generate a Bitcoin Address for the Canister

Bitcoin has different types of addresses (e.g. P2PKH, P2SH). Most of these addresses can be generated from an ECDSA public key. Currently, you can generate a P2PKH address via the following command:

```bash
dfx canister call inscriber get_p2pkh_address
```

The above command will generate a unique Bitcoin address from the ECDSA public key of the canister.

## Step 3: Send bitcoins to Canister's P2PKH Address

Now that the canister is deployed and you have a Bitcoin address, you need to top up its balance so it can send transactions.

```bash
docker exec -it <BITCOIND-CONTAINER-ID> bitcoin-cli -regtest generatetoaddress 101 <CANISTER-BITCOIN-ADDRESS>
```

Replace `CANISTER-BITCOIN-ADDRESS` with the address returned from the `get_p2pkh_address` call. Replace `BITCOIN-CONTAINER-ID` with the Docker container ID for `bitcoind`. (You can retrieve this by running `docker container ls -a` to see all running containers, and then copy the one for `bitcoind`).

## Step 4: Check the Canister's bitcoin Balance

You can check a Bitcoin address's balance by using the `get_balance` endpoint on the canister via:

```bash
dfx canister call inscriber get_balance '("BITCOIN-ADDRESS")'
```

## Step 5: Retrieve UTXOs for Canister's (or any Bitcoin) Address

You can get a Bitcoin address's UTXOs by using the `get_utxos` endpoint on the canister via:

```bash
dfx canister call inscriber get_utxos '("BITCOIN-ADDRESS")'
```

Checking the balance of a Bitcoin address relies on the [bitcoin_get_balance](https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-bitcoin_get_balance) API.

## Step 6: Inscribe and Send a Sat

<**NOTE: 95% complete; needs finetuning. WIP**>

To inscribe a BRC20 `deploy` function onto a Satoshi, for example, you can call the canister's `inscribe` endpoint via:

```bash
dfx canister call inscriber inscribe '(variant { Brc20 }, "{\"p\": \"brc-20\",\"op\":\"deploy\",\"tick\":\"demo\",\"max\":\"1000\",\"lim\":\"10\",\"dec\":\"8\"}", null, null)'
```

This effectively inscribes the following JSON-encoded data structure:

```json
{ 
    "p": "brc-20",     // protocol,
    "op": "deploy",    // function
    "tick": "demo",    // name of token
    "max": "1000",     // total supply
    "lim": "10",       // the max a user can mint
    "dec": "8"         // number of decimals
}
```

The `inscribe` endpoint has the following signature:

```rust
/// Inscribes and sends the given amount of bitcoin from this canister to the given address.
/// Returns the commit and reveal transaction IDs.
#[update]
pub async fn inscribe(
    &mut self,
    inscription_type: Protocol,
    inscription: String,
    dst_address: Option<String>,
    multisig: Option<(usize, usize)>
) -> (String, String)
```

which is why the above call has `null` arguments for the `dst_address` and `multisig` optional parameters.
