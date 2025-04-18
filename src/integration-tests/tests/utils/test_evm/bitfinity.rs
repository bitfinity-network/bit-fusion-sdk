#[cfg(feature = "dfx_tests")]
mod dfx;
mod init;
#[cfg(feature = "pocket_ic_integration_test")]
mod pocket_ic;

use std::sync::Arc;

use bridge_did::evm_link::EvmLink;
use candid::Principal;
use did::{BlockNumber, Bytes, H160, H256, Transaction, TransactionReceipt, U256};
use evm_canister_client::EvmCanisterClient;
use ic_canister_client::CanisterClient;

use super::TestEvm;
use crate::utils::error::{Result as TestResult, TestError};

#[derive(Clone)]
pub struct BitfinityEvm<C>
where
    C: CanisterClient,
{
    evm: Principal,
    signature: Principal,
    evm_client: Arc<EvmCanisterClient<C>>,
}

#[async_trait::async_trait]
impl<C> TestEvm for BitfinityEvm<C>
where
    C: CanisterClient + Send + Sync,
{
    async fn stop(&self) {}

    fn evm(&self) -> Principal {
        self.evm
    }

    fn signature(&self) -> Principal {
        self.signature
    }

    async fn chain_id(&self) -> TestResult<u64> {
        let res = self.evm_client.eth_chain_id().await?;

        Ok(res)
    }

    fn link(&self) -> EvmLink {
        EvmLink::Ic(self.evm)
    }

    async fn mint_native_tokens(&self, address: H160, amount: U256) -> TestResult<()> {
        self.evm_client
            .admin_mint_native_tokens(address, amount)
            .await??;

        Ok(())
    }

    async fn send_raw_transaction(&self, transaction: Transaction) -> TestResult<H256> {
        let res =
            self.evm_client
                .send_raw_transaction(transaction.try_into().map_err(|e| {
                    TestError::Generic(format!("Failed to convert transaction: {}", e))
                })?)
                .await??;

        Ok(res)
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
        let res = self
            .evm_client
            .eth_call(from, to, value, gas_limit, gas_price, data)
            .await??;

        hex::decode(res.trim_start_matches("0x"))
            .map_err(|e| TestError::Ganache(format!("Failed to parse result: {:?}", e)))
    }

    async fn eth_get_balance(&self, address: &H160, block: BlockNumber) -> TestResult<U256> {
        let res = self
            .evm_client
            .eth_get_balance(address.clone(), block)
            .await??;

        Ok(res)
    }

    async fn get_transaction_receipt(&self, hash: &H256) -> TestResult<Option<TransactionReceipt>> {
        let res = self
            .evm_client
            .eth_get_transaction_receipt(hash.clone())
            .await?;

        Ok(res)
    }

    async fn get_next_nonce(&self, address: &H160) -> TestResult<U256> {
        Ok(self.evm_client.account_basic(address.clone()).await?.nonce)
    }

    fn live(&self) -> bool {
        false
    }
}
