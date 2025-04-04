mod bitfinity;
mod ganache;
#[cfg(any(feature = "pocket_ic_integration_test", feature = "dfx_tests"))]
mod wrapper;

use bridge_did::evm_link::EvmLink;
use candid::Principal;
use did::{BlockNumber, Bytes, Transaction, TransactionReceipt, H160, H256, U256};

pub use self::bitfinity::BitfinityEvm;
pub use self::ganache::GanacheEvm;
#[cfg(feature = "pocket_ic_integration_test")]
pub use self::wrapper::test_evm_pocket_ic;
#[cfg(any(feature = "pocket_ic_integration_test", feature = "dfx_tests"))]
pub use self::wrapper::{test_evm, Evm, Side as EvmSide};
use crate::utils::error::Result as TestResult;

/// An abstraction layer for interacting with a test EVM node
#[async_trait::async_trait]
pub trait TestEvm: Send + Sync {
    async fn stop(&self);

    /// returns whether requires live mode
    fn live(&self) -> bool;

    /// Get the chain ID
    async fn chain_id(&self) -> TestResult<u64>;

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
    ) -> TestResult<Vec<u8>>;

    /// Get the balance of an address
    async fn eth_get_balance(&self, address: &H160, block: BlockNumber) -> TestResult<U256>;

    /// Get a transaction receipt
    async fn get_transaction_receipt(&self, hash: &H256) -> TestResult<Option<TransactionReceipt>>;

    /// Get the next nonce for an address
    async fn get_next_nonce(&self, address: &H160) -> TestResult<U256>;

    /// Evm principal
    fn evm(&self) -> Principal;

    /// Signature principal
    fn signature(&self) -> Principal;
}
