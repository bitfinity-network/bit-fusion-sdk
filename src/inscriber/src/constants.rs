use core::cell::RefCell;
use std::cell::Cell;

use ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;

thread_local! {
    pub(crate) static BITCOIN_NETWORK: Cell<BitcoinNetwork> = Cell::new(BitcoinNetwork::Regtest);

    pub(crate) static ECDSA_DERIVATION_PATH: Vec<Vec<u8>> = vec![];

    pub(crate) static ECDSA_KEY_NAME: RefCell<String> = RefCell::new(String::from(""));
}

// The fee for the `sign_with_ecdsa` endpoint using the test key.
pub(crate) const SIGN_WITH_ECDSA_COST_CYCLES: u64 = 10_000_000_000;

// The fees for the various bitcoin endpoints.
pub(crate) const GET_BALANCE_COST_CYCLES: u64 = 100_000_000;
pub(crate) const GET_UTXOS_COST_CYCLES: u64 = 10_000_000_000;
pub(crate) const GET_CURRENT_FEE_PERCENTILES_CYCLES: u64 = 100_000_000;
pub(crate) const SEND_TRANSACTION_BASE_CYCLES: u64 = 5_000_000_000;
pub(crate) const SEND_TRANSACTION_PER_BYTE_CYCLES: u64 = 20_000_000;
