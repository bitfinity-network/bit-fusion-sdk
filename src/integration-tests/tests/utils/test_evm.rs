mod bitfinity;
mod ganache;

use std::sync::Arc;

use bridge_did::evm_link::EvmLink;
use did::{BlockNumber, Bytes, Transaction, TransactionReceipt, H160, H256, U256};

pub use self::bitfinity::BitfinityEvm;
pub use self::ganache::GanacheEvm;
use crate::utils::error::Result as TestResult;

/// An abstraction layer for interacting with a test EVM node
#[async_trait::async_trait]
pub trait TestEvm: Send + Sync {
    /// Get the chain ID
    async fn eth_chain_id(&self) -> TestResult<u64>;

    /// Get EVM Link for this evm
    fn link(&self) -> EvmLink;

    /// Mint native tokens to an address
    async fn mint_native_tokens(&self, address: H160, amount: U256) -> TestResult<()>;

    /// Send a raw transaction
    async fn send_raw_transaction(&self, transaction: Transaction) -> TestResult<H256>;

    /// Call a contract
    async fn eth_call(
        &self,
        from: Option<H160>,
        to: Option<H160>,
        value: Option<U256>,
        gas_limit: u64,
        gas_price: Option<U256>,
        data: Option<Bytes>,
    ) -> TestResult<String>;

    /// Get the balance of an address
    async fn eth_get_balance(&self, address: &H160, block: BlockNumber) -> TestResult<U256>;

    /// Get a transaction receipt
    async fn get_transaction_receipt(&self, hash: &H256) -> TestResult<Option<TransactionReceipt>>;

    /// Get the next nonce for an address
    async fn get_next_nonce(&self, address: &H160) -> TestResult<U256>;
}

/// Create a default EVM instance
pub async fn default_evm() -> Arc<GanacheEvm> {
    Arc::new(GanacheEvm::run().await)
}
