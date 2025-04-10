use alloy::consensus::TxLegacy;
use alloy::core::primitives::{Address, BlockNumber as EthBlockNumber, U256};
use alloy::primitives::TxKind;
use alloy::rpc::types::Log;
use alloy_sol_types::{SolCall, SolEvent};
pub use bridge_did::batch_mint_result::{BatchMintErrorCode, BatchMintResultError};
use bridge_did::error::{BTFResult, Error};
use bridge_did::event_data::*;
use candid::CandidType;
use ethereum_json_rpc_client::{Client, EthGetLogsParams, EthJsonRpcClient};
use serde::{Deserialize, Serialize};

use crate::BTFBridge;

const DEFAULT_TX_GAS_LIMIT: u64 = 3_000_000;

/// Emitted when token is burnt or minted by BTFBridge.
#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub enum BridgeEvent {
    Burnt(BurntEventData),
    Minted(MintedEventData),
    Notify(NotifyMinterEventData),
}

impl BridgeEvent {
    pub async fn collect(
        evm_client: &EthJsonRpcClient<impl Client>,
        from_block: u64,
        to_block: u64,
        bridge_contract: Address,
    ) -> BTFResult<Vec<Self>> {
        let logs_result =
            Self::collect_logs(evm_client, from_block, to_block, bridge_contract).await;

        let logs = match logs_result {
            Ok(l) => l,
            Err(e) => {
                log::warn!("failed to collect evm logs: {e}");
                return Err(Error::EvmRequestFailed(e.to_string()));
            }
        };

        log::debug!("Got evm logs between blocks {from_block} and {to_block}: {logs:?}",);

        let events = logs
            .into_iter()
            .filter_map(|log| match BridgeEvent::from_log(log) {
                Ok(l) => Some(l),
                Err(e) => {
                    log::warn!("failed to decode log into event: {e}");
                    None
                }
            })
            .collect();
        Ok(events)
    }

    pub async fn collect_logs(
        evm_client: &EthJsonRpcClient<impl Client>,
        mut from_block: u64,
        to_block: u64,
        bridge_contract: Address,
    ) -> Result<Vec<Log>, anyhow::Error> {
        const DEFAULT_BLOCKS_TO_COLLECT_PER_PAGE: u64 = 128;
        log::debug!("collecting logs from {from_block} to {to_block} for {bridge_contract}",);

        let mut offset = DEFAULT_BLOCKS_TO_COLLECT_PER_PAGE;
        let mut logs = Vec::new();

        while from_block <= to_block {
            let to_block_for_page = (from_block + offset).min(to_block);
            log::debug!("collecting logs from {from_block} to {to_block_for_page}");
            match Self::collect_logs_from_to(
                evm_client,
                bridge_contract,
                from_block,
                to_block_for_page,
            )
            .await
            {
                Ok(new_logs) => {
                    logs.extend(new_logs);
                    // offset is inclusive, so we need to add 1
                    from_block = to_block_for_page + 1;
                    // reset offset to default value
                    offset = DEFAULT_BLOCKS_TO_COLLECT_PER_PAGE;
                }
                Err(err) => {
                    log::error!(
                        "failed to collect logs from {from_block} to {to_block_for_page}: {}",
                        err
                    );
                    // reduce offset to retry fetching logs; if offset is 0, skip the block
                    if offset > 0 {
                        offset /= 2;
                    } else {
                        log::error!("unable to collect logs for block {from_block}. Skipping it.");
                        from_block += 1;
                    }
                }
            }
        }

        Ok(logs)
    }

    /// Collects logs from the given range of blocks.
    async fn collect_logs_from_to(
        evm_client: &EthJsonRpcClient<impl Client>,
        bridge_contract: Address,
        from_block: EthBlockNumber,
        to_block: EthBlockNumber,
    ) -> Result<Vec<Log>, anyhow::Error> {
        let params = EthGetLogsParams {
            address: Some(vec![bridge_contract.into()]),
            from_block: from_block.into(),
            to_block: to_block.into(),
            topics: Some(vec![vec![
                BurnTokenEvent::SIGNATURE_HASH.0.into(),
                MintTokenEvent::SIGNATURE_HASH.0.into(),
                NotifyMinterEvent::SIGNATURE_HASH.0.into(),
            ]]),
        };
        evm_client.get_logs(params).await
    }

    pub fn from_log(log: Log) -> anyhow::Result<Self> {
        Self::try_from(log)
    }
}

impl TryFrom<Log> for BridgeEvent {
    type Error = anyhow::Error;

    fn try_from(value: Log) -> Result<Self, Self::Error> {
        let log = value.data();

        let event = BurnTokenEvent::decode_log_data(log, true)
            .map(|event| Self::Burnt(event.into()))
            .or_else(|_| {
                MintTokenEvent::decode_log_data(log, true).map(|event| Self::Minted(event.into()))
            })
            .or_else(|_| {
                NotifyMinterEvent::decode_log_data(log, true)
                    .map(|event| Self::Notify(event.into()))
            })?;

        Ok(event)
    }
}

/// Parameters for EVM transaction.
#[derive(Debug, Clone)]
pub struct TxParams {
    pub sender: Address,
    pub bridge: Address,
    pub nonce: u64,
    pub gas_price: U256,
    pub chain_id: u64,
}

/// Sends transaction with given params to call `batchMint` function
/// in Btfbridge contract.
pub fn batch_mint_transaction(
    params: TxParams,
    mint_orders_data: &[u8],
    signature: &[u8],
    orders_to_process: &[u32],
) -> TxLegacy {
    let data = BTFBridge::batchMintCall {
        encodedOrders: mint_orders_data.to_vec().into(),
        signature: signature.to_vec().into(),
        ordersToProcess: orders_to_process.into(),
    }
    .abi_encode();

    TxLegacy {
        chain_id: Some(params.chain_id),
        nonce: params.nonce,
        gas_price: params.gas_price.to(),
        gas_limit: DEFAULT_TX_GAS_LIMIT,
        to: TxKind::Call(params.bridge),
        value: U256::ZERO,
        input: data.into(),
    }
}

/// Parse the output (slice of [`u8`]) of the `batchMint` function call to a [`Vec`] of [`BatchMintResult`].
pub fn batch_mint_result(output: &[u8]) -> Result<Vec<BatchMintErrorCode>, BatchMintResultError> {
    let output = BTFBridge::batchMintCall::abi_decode_returns(output, true)?;

    output
        ._0
        .into_iter()
        .map(BatchMintErrorCode::try_from)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use alloy::primitives::Bytes;
    use alloy::rpc::types::RawLog;
    use alloy_sol_types::private::{Address, FixedBytes, Uint};
    use did::H256;

    use super::*;

    #[test]
    fn convert_raw_log_into_minted_event() {
        let bytes20 = FixedBytes([41; 20]);
        let bytes32 = FixedBytes([42; 32]);
        let addr = Address(bytes20);

        let event = MintTokenEvent {
            amount: did::U256::from(1u64).into(),
            fromToken: bytes32,
            senderID: bytes32,
            toERC20: addr,
            recipient: addr,
            nonce: 32,
            chargedFee: did::U256::from(2u64).into(),
        };
        let data = event.encode_data();
        let topic = event.topics().0;

        let raw = RawLog {
            topics: vec![H256::from_slice(&topic.0).into()],
            data: data.into(),
            address: addr,
        };

        let topics = raw
            .topics
            .iter()
            .map(|topic| topic.0.into())
            .collect::<Vec<FixedBytes<32>>>();

        let decoded_event =
            MintTokenEvent::decode_raw_log(topics, raw.data.as_ref(), true).unwrap();

        assert_eq!(event.amount, decoded_event.amount);
        assert_eq!(event.fromToken, decoded_event.fromToken);
        assert_eq!(event.senderID, decoded_event.senderID);
        assert_eq!(event.toERC20, decoded_event.toERC20);
        assert_eq!(event.recipient, decoded_event.recipient);
        assert_eq!(event.nonce, decoded_event.nonce);
        assert_eq!(event.chargedFee, decoded_event.chargedFee);
    }

    #[test]
    fn convert_raw_log_into_burnt_event() {
        let event = BurnTokenEvent {
            sender: Address::repeat_byte(1),
            amount: Uint::ZERO,
            fromERC20: Address::repeat_byte(2),
            recipientID: Bytes::default(),
            toToken: FixedBytes::from([3; 32]),
            operationID: 1,
            name: FixedBytes::from([2; 32]),
            symbol: FixedBytes::from([1; 16]),
            decimals: 18,
            memo: FixedBytes::from([1; 32]),
        };

        let raw_data = event.encode_data();

        let raw = RawLog {
            topics: vec![
                H256::from_hex_str(
                    "0x9d727a82f0ec1a11adcfae675deaa61c758c84fb0c3c9af9c05838e693e3e643",
                )
                .expect("invalid topic")
                .0,
            ],
            data: raw_data.into(),
            address: event.sender,
        };
        let topics = raw
            .topics
            .iter()
            .map(|topic| topic.0.into())
            .collect::<Vec<FixedBytes<32>>>();

        let event = BurnTokenEvent::decode_raw_log(topics, raw.data.as_ref(), true)
            .expect("failed to decode event");
        assert_eq!(event.sender, event.sender);
    }

    #[tokio::test]
    async fn test_should_get_paginated_logs() {
        env_logger::init();
        // fill logs with from 200 to 1_000 blocks (total 800 blocks);
        // set error for block 802
        let mut logs = HashMap::new();
        for block in 200..=1000 {
            logs.insert(
                block,
                vec![Log {
                    block_hash: None,
                    block_number: None,
                    transaction_hash: None,
                    transaction_index: None,
                    log_index: None,
                    removed: false,
                    block_timestamp: None,
                    inner: alloy::primitives::Log {
                        address: Address::ZERO,
                        data: alloy::primitives::LogData::new_unchecked(
                            vec![],
                            Bytes::from(vec![]),
                        ),
                    },
                }],
            );
        }

        let client = FakeEthJsonRpcClient {
            logs,
            error: Some(802),
        };
        let evm_client = EthJsonRpcClient::new(client);

        // get from 0 to 100
        let logs = BridgeEvent::collect_logs(&evm_client, 0, 100, Address::ZERO)
            .await
            .unwrap();
        assert_eq!(logs.len(), 0);

        // get from 80 to 220 (first result will be empty)
        let logs = BridgeEvent::collect_logs(&evm_client, 80, 220, Address::ZERO)
            .await
            .unwrap();
        assert_eq!(logs.len(), 21);

        // get from 100 to 800 (multiple requests)
        let logs = BridgeEvent::collect_logs(&evm_client, 100, 800, Address::ZERO)
            .await
            .unwrap();
        assert_eq!(logs.len(), 601);

        // get error block
        let logs = BridgeEvent::collect_logs(&evm_client, 801, 950, Address::ZERO)
            .await
            .unwrap();
        assert_eq!(logs.len(), 950 - 801); // error will be skipped

        // get with more blocks than available
        let logs = BridgeEvent::collect_logs(&evm_client, 10, 2000, Address::ZERO)
            .await
            .unwrap();
        assert_eq!(logs.len(), 800);
    }

    #[derive(Clone)]
    struct FakeEthJsonRpcClient {
        /// block number -> logs
        logs: HashMap<u64, Vec<Log>>,
        error: Option<u64>,
    }

    impl Client for FakeEthJsonRpcClient {
        fn send_rpc_request(
            &self,
            request: jsonrpc_core::Request,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = anyhow::Result<jsonrpc_core::Response>> + Send>,
        > {
            // get block number for eth_getLogs request
            let (id, from_block, to_block) = match request {
                jsonrpc_core::Request::Single(jsonrpc_core::Call::MethodCall(method_call)) => {
                    match method_call.params {
                        jsonrpc_core::Params::Array(params) => {
                            let obj = params[0].as_object().unwrap();
                            let from_block = obj.get("fromBlock").unwrap();
                            let to_block = obj.get("toBlock").unwrap();

                            let to_block = match to_block.as_str().unwrap() {
                                "latest" => u64::MAX,
                                _ => u64::from_str_radix(
                                    to_block.as_str().unwrap().trim_start_matches("0x"),
                                    16,
                                )
                                .unwrap(),
                            };

                            (
                                method_call.id,
                                u64::from_str_radix(
                                    from_block.as_str().unwrap().trim_start_matches("0x"),
                                    16,
                                )
                                .unwrap(),
                                to_block,
                            )
                        }
                        params => unimplemented!("expected array params: {params:?}"),
                    }
                }
                _ => unimplemented!("expected single method call request"),
            };

            let mut logs = vec![];
            let max_block = self.logs.keys().max().cloned().unwrap_or(0);
            let to_block = to_block.min(max_block);
            log::warn!("from_block: {}, to_block: {}", from_block, to_block);
            for block_number in from_block..=to_block {
                if Some(block_number) == self.error {
                    return Box::pin(async {
                        Ok(jsonrpc_core::Response::Single(
                            jsonrpc_core::Output::Failure(jsonrpc_core::Failure {
                                jsonrpc: None,
                                error: jsonrpc_core::Error {
                                    code: jsonrpc_core::ErrorCode::ServerError(-32000),
                                    message: "fake error".to_string(),
                                    data: None,
                                },
                                id,
                            }),
                        ))
                    });
                }
                if let Some(block_logs) = self.logs.get(&block_number) {
                    logs.extend_from_slice(block_logs);
                }
            }

            let response = jsonrpc_core::Response::Single(jsonrpc_core::Output::Success(
                jsonrpc_core::Success {
                    jsonrpc: None,
                    result: serde_json::json!(logs),
                    id,
                },
            ));

            Box::pin(async { Ok(response) })
        }
    }
}
