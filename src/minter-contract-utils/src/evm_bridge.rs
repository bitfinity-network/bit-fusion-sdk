use core::fmt;

use candid::CandidType;
use did::{H160, U256};
use ethereum_json_rpc_client::{Client, EthJsonRpcClient};
use ethers_core::types::{BlockNumber, U256 as EthU256};
use serde::{Deserialize, Serialize};

use crate::evm_link::EvmLink;

/// Determined side of the bridge.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub enum BridgeSide {
    Base = 0,
    Wrapped = 1,
}

impl BridgeSide {
    pub fn other(self) -> Self {
        match self {
            Self::Base => Self::Wrapped,
            Self::Wrapped => Self::Base,
        }
    }
}

impl fmt::Display for BridgeSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Base => write!(f, "Base"),
            Self::Wrapped => write!(f, "Wrapped"),
        }
    }
}

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

    /// Queries EVM params from EVM using the client.
    /// Nonce will be queried for the given address.
    pub async fn query(
        evm_client: EthJsonRpcClient<impl Client>,
        address: H160,
    ) -> anyhow::Result<Self> {
        let chain_id = evm_client.get_chain_id().await?;
        let next_block = evm_client.get_block_number().await?;
        let nonce = evm_client
            .get_transaction_count(address.0, BlockNumber::Latest)
            .await?;

        // TODO: Improve gas price selection strategy. https://infinityswap.atlassian.net/browse/EPROD-738
        let latest_block = evm_client
            .get_full_block_by_number(BlockNumber::Latest)
            .await?;
        let (tx_with_price_count, sum_price) = latest_block
            .transactions
            .iter()
            .filter_map(|tx| tx.gas_price)
            .fold((0u64, EthU256::zero()), |(count, sum), price| {
                (count + 1, sum + price)
            });

        let mean_price = sum_price / EthU256::from(tx_with_price_count.max(1));

        Ok(Self {
            chain_id,
            next_block,
            nonce,
            gas_price: mean_price.into(),
        })
    }
}
