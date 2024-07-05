use std::collections::HashMap;

use anyhow::anyhow;
use did::BlockNumber;
use ethereum_json_rpc_client::{Client, EthJsonRpcClient};
use ethers_core::types::H160;
use jsonrpc_core::{
    serde_json, Call, Id, MethodCall, Output, Params, Request, Response, Value, Version,
};
use serde::de::DeserializeOwned;

pub const CHAINID_ID: &str = "chainID";
pub const GAS_PRICE_ID: &str = "gasPrice";
pub const LATEST_BLOCK_ID: &str = "latestBlock";
pub const NONCE_ID: &str = "nonce";

/// Represents different types of queries that can be made to an EVM node
pub enum QueryType {
    GasPrice,
    Nonce { address: H160 },
    LatestBlock,
    ChainID,
}

impl QueryType {
    fn to_method_call(&self) -> Call {
        let (method, params, id) = match self {
            QueryType::GasPrice => ("eth_gasPrice", vec![], GAS_PRICE_ID),
            QueryType::Nonce { address } => (
                "eth_getTransactionCount",
                vec![
                    serde_json::to_value(address).expect("should be able to convert"),
                    serde_json::to_value(BlockNumber::Pending).expect("should be able to convert"),
                ],
                NONCE_ID,
            ),
            QueryType::LatestBlock => ("eth_blockNumber", vec![], LATEST_BLOCK_ID),
            QueryType::ChainID => ("eth_chainId", vec![], CHAINID_ID),
        };

        Call::MethodCall(MethodCall {
            jsonrpc: Some(Version::V2),
            method: method.into(),
            params: Params::Array(params),
            id: Id::Str(id.into()),
        })
    }
}

/// Simplifies the process of sending batch requests and handling responses
pub async fn batch_query(
    client: &EthJsonRpcClient<impl Client>,
    queries: &[QueryType],
) -> anyhow::Result<HashMap<Id, Value>> {
    let calls = queries
        .iter()
        .map(QueryType::to_method_call)
        .collect::<Vec<_>>();
    let request = Request::Batch(calls);
    let Response::Batch(responses) = client.request(request).await? else {
        return Err(anyhow!("Unexpected response format"));
    };

    let mut response_map = HashMap::new();
    for response in responses {
        if let Output::Success(success) = response {
            response_map.insert(success.id, success.result);
        } else {
            return Err(anyhow!("Failed to process response"));
        }
    }

    Ok(response_map)
}

/// A helper trait to simplify querying the response by id
pub trait Query {
    /// Get a value from the response by its id
    fn get_value_by_id<R: DeserializeOwned>(&self, id: Id) -> anyhow::Result<R>;
}

impl Query for HashMap<Id, Value> {
    fn get_value_by_id<R: DeserializeOwned>(&self, id: Id) -> anyhow::Result<R> {
        let value = self
            .get(&id)
            .ok_or_else(|| anyhow!("Field not found in response"))?;

        let value = serde_json::from_value(value.clone())?;

        Ok(value)
    }
}
