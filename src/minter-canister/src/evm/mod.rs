use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use did::{Bytes, Transaction, TransactionReceipt, H160, H256, U256};
use minter_did::error::Error;

use crate::context::Context;

mod evm_client;
#[cfg(test)]
mod evm_client_mock;

pub use evm_client::{EvmCanister, EvmCanisterImpl};

/// Interface for calling EVMC methods
#[cfg_attr(test, mockall::automock)]
#[async_trait(?Send)]
pub trait Evm {
    /// Sends a raw signed transaction from arbitrary address
    async fn send_raw_transaction(
        &self,
        tx: Transaction,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<H256, Error>;

    /// Returns the contract code from the given address
    async fn get_contract_code(
        &self,
        address: H160,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<Vec<u8>, Error>;

    /// Returns the balance for the given address
    async fn get_balance(
        &self,
        address: H160,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<U256, Error>;

    /// Returns the transaction by the given hash
    async fn get_transaction_by_hash(
        &self,
        tx_hash: H256,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<Option<Transaction>, Error>;

    /// Returns the transaction receipt by the transaction hash
    async fn get_transaction_receipt_by_hash(
        &self,
        tx_hash: H256,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<Option<TransactionReceipt>, Error>;

    /// Returns the transaction count for the given address
    async fn get_transaction_count(
        &self,
        address: H160,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<U256, Error>;

    /// Performs read-only transaction.
    #[allow(clippy::too_many_arguments)]
    async fn eth_call(
        &self,
        from: Option<H160>,
        to: Option<H160>,
        value: Option<U256>,
        gas_limit: u64,
        gas_price: Option<U256>,
        data: Option<Bytes>,
        context: &Rc<RefCell<dyn Context>>,
    ) -> Result<String, Error>;

    /// Returns chain id of the canister
    async fn eth_chain_id(&self, context: &Rc<RefCell<dyn Context>>) -> Result<u64, Error>;
}
