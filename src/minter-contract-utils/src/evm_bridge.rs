use core::fmt;
use std::collections::HashMap;

use anyhow::anyhow;
use candid::CandidType;
use did::{H160, H256, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ethereum_json_rpc_client::{Client, EthGetLogsParams, EthJsonRpcClient};
use ethers_core::types::{BlockNumber, Log, U256 as EthU256};
use jsonrpc_core::{serde_json, Call, Id, MethodCall, Output, Params, Request, Response, Version};
use serde::{Deserialize, Serialize};

use crate::bft_bridge_api::{self, BURNT_EVENT, MINTED_EVENT};
use crate::build_data::BFT_BRIDGE_SMART_CONTRACT_CODE;
use crate::evm_link::EvmLink;

macro_rules! make_params_array {
    ($($items:expr),*) => {
        Params::Array(vec![$(serde_json::to_value($items)?, )*])
    };
}

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
        const DEFAULT_GAS_PRICE: u64 = 46 * 10u64.pow(9);
        let gas_price = if mean_price == EthU256::zero() {
            DEFAULT_GAS_PRICE.into()
        } else {
            mean_price.into()
        };

        Ok(Self {
            chain_id,
            next_block,
            nonce,
            gas_price,
        })
    }
}

/// Status of BftBridge contract initialization
#[derive(Debug, Clone, Default, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub enum BftBridgeContractStatus {
    #[default]
    None,
    Creating(EvmLink, H256),
    Created(H160),
}

impl BftBridgeContractStatus {
    /// Starts Contract initialization if currenst status is None.
    /// Else, returns an error.
    pub async fn initialize(
        &mut self,
        link: EvmLink,
        chain_id: u32,
        signer: impl TransactionSigner,
        minter_address: H160,
    ) -> anyhow::Result<H256> {
        match self {
            BftBridgeContractStatus::None => {}
            BftBridgeContractStatus::Creating(_, _) => {
                return Err(anyhow!("creation of BftBridge contract already started"))
            }
            BftBridgeContractStatus::Created(_) => {
                return Err(anyhow!("creation of BftBridge contract already finised"))
            }
        };

        let client = link.get_json_rpc_client();
        let sender = signer.get_address().await?.0;
        let nonce = client
            .get_transaction_count(sender, BlockNumber::Latest)
            .await?;
        let gas_price = client.gas_price().await?;
        let mut transaction = bft_bridge_api::deploy_transaction(
            sender,
            nonce.into(),
            gas_price,
            chain_id,
            BFT_BRIDGE_SMART_CONTRACT_CODE.clone(),
            minter_address.into(),
        );
        let signature = signer.sign_transaction(&(&transaction).into()).await?;

        transaction.r = signature.r.0;
        transaction.s = signature.s.0;
        transaction.v = signature.v.0;
        transaction.hash = transaction.hash();

        let hash = client.send_raw_transaction(transaction).await?;

        *self = BftBridgeContractStatus::Creating(link, hash.into());
        Ok(hash.into())
    }

    /// Refreshes the status of the BftBridge contract.
    /// If current status is `Creating`, tries to get the creation tx result.
    /// If current status is `None`, returns an error.
    /// If current status is `Created`, returns Ok.
    pub async fn refresh(&mut self) -> Result<(), anyhow::Error> {
        match self.clone() {
            BftBridgeContractStatus::None => {
                Err(anyhow!("creation of BftBridge contract not started"))
            }
            BftBridgeContractStatus::Creating(link, tx) => {
                let address = Self::check_creation_tx(link, tx).await?;
                *self = BftBridgeContractStatus::Created(address);
                Ok(())
            }
            BftBridgeContractStatus::Created(_) => Ok(()),
        }
    }

    async fn check_creation_tx(link: EvmLink, tx: H256) -> Result<H160, anyhow::Error> {
        let client = link.get_json_rpc_client();
        let receipt = client.get_receipt_by_hash(tx.0).await?;
        let contract = receipt
            .contract_address
            .ok_or_else(|| anyhow!("BftBridge contract not present in receipt"))?;
        Ok(contract.into())
    }

    /// Returns `true` if the bft bridge contract status is [`Created`].
    ///
    /// [`Created`]: BftBridgeContractStatus::Created
    #[must_use]
    pub fn is_created(&self) -> bool {
        matches!(self, Self::Created(..))
    }

    /// Returns `true` if the bft bridge contract status is [`None`].
    ///
    /// [`None`]: BftBridgeContractStatus::None
    #[must_use]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Returns `true` if the bft bridge contract status is [`Creating`].
    ///
    /// [`Creating`]: BftBridgeContractStatus::Creating
    #[must_use]
    pub fn is_creating(&self) -> bool {
        matches!(self, Self::Creating(..))
    }
}

/// `EvmData` contains data related to an Ethereum Virtual Machine (EVM)
/// transaction
pub struct EvmData {
    pub nonce: U256,
    pub gas_price: U256,
    pub events: Vec<Log>,
}

impl EvmData {
    /// Queries the EVM bridge contract for relevant events within the specified block range.
    ///
    /// # Arguments
    /// - `evm_client`: The EVM client to use for making the RPC requests.
    /// - `bridge_contract`: The address of the bridge contract to query.
    /// - `address`: The address to query the nonce and gas price for.
    /// - `from_block`: The starting block number to query events from.
    /// - `to_block`: The ending block number to query events to.
    ///
    /// # Returns
    /// An `EvmData` struct containing the nonce, gas price, and relevant events.
    pub async fn query(
        evm_client: EvmLink,
        bridge_contract: H160,
        address: H160,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> anyhow::Result<Self> {
        let evm_client = evm_client.get_client();

        let calls = vec![
            Call::MethodCall(MethodCall {
                jsonrpc: Some(Version::V2),
                method: "eth_getTransactionCount".into(),
                params: make_params_array!(address.0, BlockNumber::Latest),
                id: Id::Str("nonce".into()),
            }),
            Call::MethodCall(MethodCall {
                jsonrpc: Some(Version::V2),
                method: "eth_gasPrice".into(),
                params: Params::Array(vec![]),
                id: Id::Str("gasPrice".into()),
            }),
            Call::MethodCall(MethodCall {
                jsonrpc: Some(Version::V2),
                method: "eth_getLogs".into(),
                params: Params::Array(vec![serde_json::to_value(EthGetLogsParams {
                    address: Some(vec![bridge_contract.into()]),
                    from_block,
                    to_block,
                    topics: Some(vec![vec![
                        BURNT_EVENT.signature(),
                        MINTED_EVENT.signature(),
                    ]]),
                })?]),
                id: Id::Str("events".into()),
            }),
        ];

        let request = Request::Batch(calls);
        let Response::Batch(responses) = evm_client.send_rpc_request(request).await? else {
            return Err(anyhow::anyhow!("Unexpected response format"));
        };

        let mut response_map = HashMap::new();
        for response in responses {
            if let Output::Success(success) = response {
                response_map.insert(success.id, success.result);
            } else {
                return Err(anyhow::anyhow!("Failed to process response"));
            }
        }

        let nonce = response_map
            .remove(&Id::Str("nonce".into()))
            .ok_or_else(|| anyhow::anyhow!("Nonce not found in response"))?;
        let nonce = serde_json::from_value(nonce)?;

        let gas_price = response_map
            .remove(&Id::Str("gasPrice".into()))
            .ok_or_else(|| anyhow::anyhow!("Gas price not found in response"))?;

        let gas_price = serde_json::from_value(gas_price)?;

        let events = response_map
            .remove(&Id::Str("events".into()))
            .ok_or_else(|| anyhow::anyhow!("Events not found in response"))?;

        let events = serde_json::from_value(events)?;

        Ok(Self {
            nonce,
            gas_price,
            events,
        })
    }
}
