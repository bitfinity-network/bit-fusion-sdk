use std::collections::HashMap;

use anyhow::anyhow;

use ethereum_json_rpc_client::{Client, EthJsonRpcClient};
use ethers_core::types::H160;
use jsonrpc_core::{Call, Id, MethodCall, Output, Params, Request, Response, Value, Version};

use jsonrpc_core::serde_json;

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
            QueryType::GasPrice => ("eth_gasPrice", vec![], "gasPrice"),
            QueryType::Nonce { address } => (
                "eth_getTransactionCount",
                vec![serde_json::to_value(address).expect("should be able to convert")],
                "nonce",
            ),
            QueryType::LatestBlock => ("eth_blockNumber", vec![], "latestBlock"),
            QueryType::ChainID => ("eth_chainId", vec![], "chainID"),
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
