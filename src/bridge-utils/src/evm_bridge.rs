use bridge_did::evm_link::EvmLink;
use candid::CandidType;
use did::{BlockNumber, H160, U256};
use ethereum_json_rpc_client::{Client, EthJsonRpcClient};
use jsonrpc_core::Id;
use serde::{Deserialize, Serialize};

use crate::btf_events::TxParams;
use crate::query::{CHAINID_ID, LATEST_BLOCK_ID, NONCE_ID, Query, QueryType, batch_query};

/// Information about EVM on a bridge side.
#[derive(Default, Debug, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub struct EvmInfo {
    pub link: EvmLink,
    pub bridge_contract: H160,
    pub params: Option<EvmParams>,
}

/// Parameters to query from EVM.
#[derive(Default, Debug, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub struct EvmParams {
    pub chain_id: u64,
    pub next_block: u64,
    pub nonce: u64,
    pub gas_price: U256,
}

impl EvmParams {
    pub fn new(chain_id: u64, next_block: u64, nonce: u64, gas_price: U256) -> Self {
        Self {
            chain_id,
            next_block,
            nonce,
            gas_price,
        }
    }

    /// Returns transaction parameters for the EVM.
    pub fn create_tx_params(&self, sender: H160, bridge: H160) -> TxParams {
        TxParams {
            sender: sender.0,
            bridge: bridge.0,
            nonce: self.nonce,
            gas_price: self.gas_price.0,
            chain_id: self.chain_id,
        }
    }

    /// Queries EVM params from EVM using the client.
    /// Nonce will be queried for the given address.
    pub async fn query(
        evm_client: EthJsonRpcClient<impl Client>,
        address: H160,
    ) -> anyhow::Result<Self> {
        let responses = batch_query(
            &evm_client,
            &[
                QueryType::ChainID,
                QueryType::LatestBlock,
                QueryType::Nonce {
                    address: address.into(),
                },
            ],
        )
        .await?;

        let chain_id: U256 = responses.get_value_by_id(Id::Str(CHAINID_ID.into()))?;
        let next_block: U256 = responses.get_value_by_id(Id::Str(LATEST_BLOCK_ID.into()))?;
        let nonce: U256 = responses.get_value_by_id(Id::Str(NONCE_ID.into()))?;

        // TODO: Improve gas price selection strategy. https://infinityswap.atlassian.net/browse/EPROD-738
        let latest_block = evm_client
            .get_full_block_by_number(BlockNumber::Latest)
            .await?;
        let (tx_with_price_count, sum_price) = latest_block
            .transactions
            .iter()
            .filter_map(|tx| tx.gas_price.clone())
            .fold((0u64, U256::zero()), |(count, sum), price| {
                (count + 1, sum + price)
            });

        let mean_price = sum_price.0 / alloy::primitives::U256::from(tx_with_price_count.max(1));

        let mean_price: U256 = mean_price.into();
        const DEFAULT_GAS_PRICE: u64 = 46 * 10u64.pow(9);
        let gas_price = if mean_price == U256::zero() {
            DEFAULT_GAS_PRICE.into()
        } else {
            mean_price
        };

        Ok(Self {
            chain_id: chain_id.0.to(),
            next_block: next_block.0.to(),
            nonce: nonce.0.to(),
            gas_price,
        })
    }
}
