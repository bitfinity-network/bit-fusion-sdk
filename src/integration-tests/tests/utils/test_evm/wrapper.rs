use std::sync::Arc;

use bridge_did::evm_link::EvmLink;
use candid::Principal;
use did::{BlockNumber, Bytes, Transaction, TransactionReceipt, H160, H256, U256};

use super::{BitfinityEvm, GanacheEvm, TestEvm};
use crate::utils::error::Result as TestResult;

const EVM_ENV_VAR: &str = "EVM";
const ENV_EVM_BITFINITY: &str = "bitfinity";
const ENV_EVM_GANACHE: &str = "ganache";

pub enum Evm {
    #[cfg(feature = "pocket_ic_integration_test")]
    Bitfinity(BitfinityEvm<ic_canister_client::PocketIcClient>),
    #[cfg(all(not(feature = "pocket_ic_integration_test"), feature = "dfx_tests"))]
    Bitfinity(BitfinityEvm<ic_canister_client::IcAgentClient>),
    Ganache(GanacheEvm),
}

/// Create a default EVM instance
pub async fn test_evm() -> Arc<Evm> {
    // get evm to use from env `EVM`
    let evm_var = std::env::var(EVM_ENV_VAR).unwrap_or_else(|_| ENV_EVM_BITFINITY.to_string());

    match evm_var.as_str() {
        #[cfg(all(not(feature = "pocket_ic_integration_test"), feature = "dfx_tests"))]
        ENV_EVM_BITFINITY => Arc::new(Evm::Bitfinity(BitfinityEvm::dfx().await)),
        #[cfg(feature = "pocket_ic_integration_test")]
        ENV_EVM_BITFINITY => unimplemented!("use test_evm_pocket_ic instead"),
        ENV_EVM_GANACHE => Arc::new(Evm::Ganache(GanacheEvm::run().await)),
        _ => panic!("Unknown EVM: {}", evm_var),
    }
}

#[cfg(feature = "pocket_ic_integration_test")]
pub async fn test_evm_pocket_ic(pocket_ic: &Arc<ic_exports::pocket_ic::PocketIc>) -> Arc<Evm> {
    // get evm to use from env `EVM`
    let evm_var = std::env::var(EVM_ENV_VAR).unwrap_or_else(|_| ENV_EVM_BITFINITY.to_string());

    match evm_var.as_str() {
        ENV_EVM_BITFINITY => Arc::new(Evm::Bitfinity(BitfinityEvm::pocket_ic(pocket_ic).await)),
        ENV_EVM_GANACHE => Arc::new(Evm::Ganache(GanacheEvm::run().await)),
        _ => panic!("Unknown EVM: {}", evm_var),
    }
}

#[async_trait::async_trait]
impl TestEvm for Evm {
    fn evm(&self) -> Principal {
        match self {
            Evm::Bitfinity(evm) => evm.evm(),
            Evm::Ganache(evm) => evm.evm(),
        }
    }

    fn signature(&self) -> Principal {
        match self {
            Evm::Bitfinity(evm) => evm.signature(),
            Evm::Ganache(evm) => evm.signature(),
        }
    }

    async fn chain_id(&self) -> TestResult<u64> {
        match self {
            Evm::Bitfinity(evm) => evm.chain_id().await,
            Evm::Ganache(evm) => evm.chain_id().await,
        }
    }

    fn link(&self) -> EvmLink {
        match self {
            Evm::Bitfinity(evm) => evm.link(),
            Evm::Ganache(evm) => evm.link(),
        }
    }

    async fn mint_native_tokens(&self, address: H160, amount: U256) -> TestResult<()> {
        match self {
            Evm::Bitfinity(evm) => evm.mint_native_tokens(address, amount).await,
            Evm::Ganache(evm) => evm.mint_native_tokens(address, amount).await,
        }
    }

    async fn send_raw_transaction(&self, transaction: Transaction) -> TestResult<H256> {
        match self {
            Evm::Bitfinity(evm) => evm.send_raw_transaction(transaction).await,
            Evm::Ganache(evm) => evm.send_raw_transaction(transaction).await,
        }
    }

    async fn eth_call(
        &self,
        from: Option<H160>,
        to: Option<H160>,
        value: Option<U256>,
        gas_limit: u64,
        gas_price: Option<U256>,
        data: Option<Bytes>,
    ) -> TestResult<Vec<u8>> {
        match self {
            Evm::Bitfinity(evm) => {
                evm.eth_call(from, to, value, gas_limit, gas_price, data)
                    .await
            }
            Evm::Ganache(evm) => {
                evm.eth_call(from, to, value, gas_limit, gas_price, data)
                    .await
            }
        }
    }

    async fn eth_get_balance(&self, address: &H160, block: BlockNumber) -> TestResult<U256> {
        match self {
            Evm::Bitfinity(evm) => evm.eth_get_balance(address, block).await,
            Evm::Ganache(evm) => evm.eth_get_balance(address, block).await,
        }
    }

    async fn get_transaction_receipt(&self, hash: &H256) -> TestResult<Option<TransactionReceipt>> {
        match self {
            Evm::Bitfinity(evm) => evm.get_transaction_receipt(hash).await,
            Evm::Ganache(evm) => evm.get_transaction_receipt(hash).await,
        }
    }

    fn live(&self) -> bool {
        match self {
            Evm::Bitfinity(evm) => evm.live(),
            Evm::Ganache(evm) => evm.live(),
        }
    }

    async fn get_next_nonce(&self, address: &H160) -> TestResult<U256> {
        match self {
            Evm::Bitfinity(evm) => evm.get_next_nonce(address).await,
            Evm::Ganache(evm) => evm.get_next_nonce(address).await,
        }
    }
}
