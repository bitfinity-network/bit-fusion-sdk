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

### Step 1: Init, Build, and Deploy

```bash
./run.sh
```

The above command will start the Bitcoin daemon in a Docker container, create a wallet called "testwallet", generate enough blocks to make sure the wallet has enough bitcoins to spend, start the local IC replica in the background, connecting it to the Bitcoin daemon in `regtest` mode, and then build and deploy the canister. You might see an error in the logs if the "testwallet" already exists, but this is not a problem.

### Step 2: Generate a Bitcoin Address for the Canister

Bitcoin has different types of addresses (e.g. P2PKH, P2SH). Most of these addresses can be generated from an ECDSA public key. Currently, you can generate the native segwit address type (`P2WPKH`) via the following command:

```bash
dfx canister call inscriber get_bitcoin_address
```

The above command will generate a unique Bitcoin address from the ECDSA public key of the canister.

### Step 3: Send bitcoins to Canister's Bitcoin Address

Now that the canister is deployed and you have a Bitcoin address, you need to top up its balance so it can send transactions. To avoid UTXO clogging, and since the Bitcoin daemon already generates enough blocks when it starts, generate only 1 additional block and effectively reward the canister wallet with about `5 BTC`. Run the following command:

```bash
docker exec -it <BITCOIND-CONTAINER-ID> bitcoin-cli -regtest generatetoaddress 1 <CANISTER-BITCOIN-ADDRESS>
```

Replace `CANISTER-BITCOIN-ADDRESS` with the address returned from the `get_bitcoin_address` call. Replace `BITCOIN-CONTAINER-ID` with the Docker container ID for `bitcoind`. (You can retrieve this by running `docker container ls -a` to see all running containers, and then copy the one for `bitcoind`).

### Step 4: Check the Canister's bitcoin Balance

You can check a Bitcoin address's balance by using the `get_balance` endpoint on the canister via:

```bash
dfx canister call inscriber get_balance '("BITCOIN-ADDRESS")'
```

### Step 5: Retrieve UTXOs for Canister's (or any Bitcoin) Address

You can get a Bitcoin address's UTXOs by using the `get_utxos` endpoint on the canister via:

```bash
dfx canister call inscriber get_utxos '("BITCOIN-ADDRESS")'
```

Checking the balance of a Bitcoin address relies on the [bitcoin_get_balance](https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-bitcoin_get_balance) API.

### Step 6: Inscribe and Send a Sat

To make an Ordinal (NFT) inscription, for example, you can call the canister's `inscribe` endpoint via:

```bash
dfx canister call inscriber inscribe '(variant { Nft }, "{\"content_type\": \"text/plain\",\"body\":\"demo\"}", "LEFTOVERS-ADDRESS", null, null)'
```

This effectively inscribes the following JSON-encoded data structure:

```json
{ 
    "content_type": "text/plain",
    "body": "demo",
}
```

To inscribe a BRC20 `deploy` function onto a Satoshi, for example, you can call the canister's `inscribe` endpoint via:

```bash
dfx canister call inscriber inscribe '(variant { Brc20 }, "{\"p\": \"brc-20\",\"op\":\"deploy\",\"tick\":\"demo\",\"max\":\"1000\",\"lim\":\"10\",\"dec\":\"8\"}", "LEFTOVERS-ADDRESS", null, null)'
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
    leftovers_address: String,
    dst_address: Option<String>,
    multisig_config: Option<Multisig>,
) -> (String, String)
```

which is why the above calls has `null` arguments for the `dst_address` and `multisig_config` optional parameters.
