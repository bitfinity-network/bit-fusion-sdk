# BRC-20 Bridge

This canister provides the mechanism for executing Bitcoin [BRC-20](https://domo-2.gitbook.io/brc-20-experiment/) inscriptions, as well as swapping them for equivalent ERC-20 tokens.

## Testing the Canister Locally

### Prerequisites

- Use `dfx` version <= `0.17.x` (for now). [Get `dfx` here](https://internetcomputer.org/docs/current/developer-docs/getting-started/install/#installing-dfx) if you don't have it already.
- [Install the Rust toolchain](https://www.rust-lang.org/tools/install) if it's not already installed.
- [Download and install Docker, with Compose](https://www.docker.com/products/docker-desktop/) if you don't already have it.

After installing Rust, add the `wasm32` target via:

```bash
rustup target add wasm32-unknown-unknown # Required for building IC canisters
```

### Init, Build, and Deploy

```bash
./scripts/brc20_bridging.sh
```

The above command will start the Bitcoin daemon in a Docker container, create a wallet called "testwallet", generate enough blocks to make sure the wallet has enough bitcoins to spend, start the local IC replica in the background, connecting it to the Bitcoin daemon in `regtest` mode, and then build and deploy the canister. You might see an error in the logs if the "testwallet" already exists, but this is not a problem.

Once the canister is deployed, you can interact with it.

### Endpoint: Generate a Bitcoin Address for the Canister

Bitcoin has different types of addresses (e.g. P2PKH, P2SH). Most of these addresses can be generated from an ECDSA public key. Currently, you can generate the native segwit address type (`P2WPKH`) via the following command:

```bash
dfx canister call brc20-bridge get_deposit_address
```

The above command will generate a unique Bitcoin address from the ECDSA public key of the canister. You can send your BRC20 inscriptions to this address.

### Endpoint: Get Inscription Fees

To get the fees to pay for your inscription, i.e., the amount of sats to deposit, you can execute:

```bash
dfx canister call brc20-bridge get_inscription_fees '(variant { Brc20 }, "{\"p\": \"brc-20\",\"op\":\"deploy\",\"tick\":\"demo\",\"max\":\"1000\",\"lim\":\"10\",\"dec\":\"8\"}", null)'
```

This will return an object containing the amount you need to deposit to accomplish the entire inscription process:

```rust
pub struct InscriptionFees {
    pub commit_fee: u64,
    pub reveal_fee: u64,
    pub transfer_fee: Option<u64>,
    pub postage: u64,
    pub leftover_amount: u64,
}
```

### Send bitcoins to Canister's Bitcoin Address

Now that the canister is deployed and you have its deposit address, you need to top up its balance so it can send transactions. To avoid UTXO clogging, and since the Bitcoin daemon already generates enough blocks when it starts, generate only 1 additional block and effectively reward the canister wallet with some BTC. Run the following command:

```bash
docker exec -it <BITCOIND-CONTAINER-ID> bitcoin-cli -regtest generatetoaddress 1 <CANISTER-BITCOIN-ADDRESS>
```

Replace `CANISTER-BITCOIN-ADDRESS` with the address returned from the `get_deposit_address` call. Replace `BITCOIN-CONTAINER-ID` with the Docker container ID for `bitcoind`. (You can retrieve this by running `docker container ls -a` to see all running containers, and then copy the one for `bitcoind`).

### Endpoint: Check Balance

You can check a Bitcoin address's balance by using the `get_balance` endpoint on the canister via:

```bash
dfx canister call brc20-bridge get_balance '("BITCOIN-ADDRESS")'
```

### Inscribe and Send a Sat

To inscribe a BRC20 `deploy` function onto a Satoshi, for example, you can call the canister's `inscribe` endpoint via:

```bash
dfx canister call brc20-bridge inscribe '(variant { Brc20 }, "{\"p\": \"brc-20\",\"op\":\"deploy\",\"tick\":\"demo\",\"max\":\"1000\",\"lim\":\"10\",\"dec\":\"8\"}", "LEFTOVERS-ADDRESS", "DST-ADDRESS", null)'
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
    dst_address: String,
    multisig_config: Option<Multisig>,
) -> InscribeResult<InscribeTransactions>
```

which is why the above calls has `null` arguments for the `multisig_config` optional parameter.

## BRC20 Transfer

The previous step can also be used to perform a BRC20 transfer. The BRC20 transfer requires an additional step which is the transfer of the reveal UTXO to the final recipient. For this reason the `brc20_transfer` endpoint must be used instead:

```bash
dfx canister call brc20-bridge brc20_transfer '("{\"p\": \"brc-20\",\"op\":\"transfer\",\"tick\":\"demo\",\"amt\":\"1000\"}", "LEFTOVERS-ADDRESS", "DST-ADDRESS", null)'
```

This effectively inscribes the following JSON-encoded data structure:

```json
{ 
    "p": "brc-20",     // protocol,
    "op": "transfer",  // function
    "tick": "demo",    // name of token
    "amt": "1000",     // amount to transfer
}
```

The endpoint has the following signature:

```rust
/// Inscribes a BRC20 transfer and sends the inscribed sat from this canister to the given address.
#[update]
pub async fn brc20_transfer(
    &mut self,
    inscription: String,
    leftovers_address: String,
    dst_address: String,
    multisig_config: Option<Multisig>,
) -> InscribeResult<Brc20TransferTransactions>;
```
