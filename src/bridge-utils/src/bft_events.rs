use std::fmt::{Display, Formatter};

use alloy_sol_types::private::{Bytes, LogData};
use alloy_sol_types::{SolCall, SolEvent};
use anyhow::anyhow;
use bridge_did::error::{BftResult, Error};
use candid::CandidType;
use ethereum_json_rpc_client::{Client, EthGetLogsParams, EthJsonRpcClient};
use ethers_core::types::{BlockNumber as EthBlockNumber, Log, Transaction, H160, U256};
use serde::{Deserialize, Serialize};

use crate::BFTBridge::{self, BurnTokenEvent, MintTokenEvent, NotifyMinterEvent};

/// Emitted when token is burnt or minted by BFTBridge.
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
        bridge_contract: H160,
    ) -> BftResult<Vec<Self>> {
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
        bridge_contract: H160,
    ) -> Result<Vec<Log>, anyhow::Error> {
        const DEFAULT_BLOCKS_TO_COLLECT_PER_PAGE: u64 = 128;
        log::debug!("collecting logs from {from_block} to {to_block}",);

        let mut offset = DEFAULT_BLOCKS_TO_COLLECT_PER_PAGE;
        let mut logs = Vec::new();

        while from_block <= to_block {
            let to_block_for_page = (from_block + offset).min(to_block);
            log::debug!("collecting logs from {from_block} to {to_block_for_page}");
            match Self::collect_logs_from_to(
                evm_client,
                bridge_contract,
                EthBlockNumber::Number(from_block.into()),
                EthBlockNumber::Number(to_block_for_page.into()),
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
        bridge_contract: H160,
        from_block: EthBlockNumber,
        to_block: EthBlockNumber,
    ) -> Result<Vec<Log>, anyhow::Error> {
        let params = EthGetLogsParams {
            address: Some(vec![bridge_contract]),
            from_block,
            to_block,
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
        let topics = value
            .topics
            .iter()
            .map(|topic| topic.0.into())
            .collect::<Vec<_>>();

        let log = LogData::new(topics, Bytes(value.data.0))
            .ok_or_else(|| anyhow!("failed to decode log"))?;

        let event = BurnTokenEvent::decode_log_data(&log, true)
            .map(|event| Self::Burnt(event.into()))
            .or_else(|_| {
                MintTokenEvent::decode_log_data(&log, true).map(|event| Self::Minted(event.into()))
            })
            .or_else(|_| {
                NotifyMinterEvent::decode_log_data(&log, true)
                    .map(|event| Self::Notify(event.into()))
            })?;

        Ok(event)
    }
}

/// Emitted when token is burnt by BFTBridge.
#[derive(Debug, Default, Clone, CandidType, Serialize, Deserialize)]
pub struct BurntEventData {
    pub sender: did::H160,
    pub amount: did::U256,
    pub from_erc20: did::H160,
    pub recipient_id: Vec<u8>,
    pub to_token: Vec<u8>,
    pub operation_id: u32,
    pub name: Vec<u8>,
    pub symbol: Vec<u8>,
    pub decimals: u8,
}

impl From<BurnTokenEvent> for BurntEventData {
    fn from(event: BurnTokenEvent) -> Self {
        Self {
            sender: event.sender.into(),
            amount: event.amount.into(),
            from_erc20: event.fromERC20.into(),
            recipient_id: event.recipientID.into(),
            to_token: event.toToken.0.into(),
            operation_id: event.operationID,
            name: event.name.0.into(),
            symbol: event.symbol.0.into(),
            decimals: event.decimals,
        }
    }
}

/// Event emitted when token is minted by BFTBridge.
#[derive(Debug, Default, Clone, CandidType, Serialize, Deserialize)]
pub struct MintedEventData {
    pub amount: did::U256,
    pub from_token: Vec<u8>,
    pub sender_id: Vec<u8>,
    pub to_erc20: did::H160,
    pub recipient: did::H160,
    pub nonce: u32,
    pub fee_charged: did::U256,
}

impl From<MintTokenEvent> for MintedEventData {
    fn from(event: MintTokenEvent) -> Self {
        Self {
            amount: event.amount.into(),
            from_token: event.fromToken.0.into(),
            sender_id: event.senderID.0.into(),
            to_erc20: event.toERC20.into(),
            recipient: event.recipient.into(),
            nonce: event.nonce,
            fee_charged: event.chargedFee.into(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, CandidType, Serialize, Deserialize)]
#[repr(u32)]
pub enum MinterNotificationType {
    DepositRequest = 1,
    RescheduleOperation = 2,
    Other,
}

impl From<u32> for MinterNotificationType {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::DepositRequest,
            2 => Self::RescheduleOperation,
            _ => Self::Other,
        }
    }
}

impl Display for MinterNotificationType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MinterNotificationType::DepositRequest => write!(f, "DepositRequest"),
            MinterNotificationType::RescheduleOperation => write!(f, "RescheduleOperation"),
            MinterNotificationType::Other => write!(f, "Other"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, CandidType, Serialize, Deserialize)]
pub struct NotifyMinterEventData {
    pub notification_type: MinterNotificationType,
    pub tx_sender: did::H160,
    pub user_data: Vec<u8>,
}

impl From<NotifyMinterEvent> for NotifyMinterEventData {
    fn from(event: NotifyMinterEvent) -> Self {
        Self {
            notification_type: event.notificationType.into(),
            tx_sender: event.txSender.into(),
            user_data: event.userData.0.into(),
        }
    }
}

pub fn mint_transaction(
    sender: H160,
    bridge: H160,
    nonce: U256,
    gas_price: U256,
    mint_order_data: &[u8],
    chain_id: u32,
) -> Transaction {
    let data = BFTBridge::mintCall {
        encodedOrder: mint_order_data.to_vec().into(),
    }
    .abi_encode();

    pub const DEFAULT_TX_GAS_LIMIT: u64 = 3_000_000;
    ethers_core::types::Transaction {
        from: sender,
        to: bridge.into(),
        nonce,
        value: U256::zero(),
        gas: DEFAULT_TX_GAS_LIMIT.into(),
        gas_price: Some(gas_price),
        input: data.into(),
        chain_id: Some(chain_id.into()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use alloy_sol_types::private::{Address, FixedBytes};
    use did::H256;
    use ethers_core::abi::{Bytes, RawLog};
    use ethers_core::utils::hex::traits::FromHex;

    use super::*;

    #[test]
    fn convert_raw_log_into_minted_event() {
        let bytes20 = FixedBytes([41; 20]);
        let bytes32 = FixedBytes([42; 32]);
        let addr = Address(bytes20);

        let event = MintTokenEvent {
            amount: did::U256::one().into(),
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
            data,
        };

        let topics = raw
            .topics
            .iter()
            .map(|topic| topic.0.into())
            .collect::<Vec<FixedBytes<32>>>();

        let decoded_event =
            MintTokenEvent::decode_raw_log(topics, &raw.data.to_vec(), true).unwrap();

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
        let raw = RawLog {
            topics: vec![H256::from_hex_str("0xfa3804fd5313cc219c6d3a833f7dbc2b1b48ac5edbae532006f1aa876a23eb79").unwrap().0],
            data: Bytes::from_hex("0x000000000000000000000000e41b09c6e9eaa79356b10f4181564b4bdb169d3500000000000000000000000000000000000000000000000000000000000003e80000000000000000000000002ea5d83d5a08d8556f726d3004a50aa8aa81c5c200000000000000000000000000000000000000000000000000000000000001200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000057617465726d656c6f6e0000000000000000000000000000000000000000000057544d0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200100056b29dc8b8e5954eebac85b3145745362adfa50d8ad9e00000000000000").unwrap(),
        };
        let topics = raw
            .topics
            .iter()
            .map(|topic| topic.0.into())
            .collect::<Vec<FixedBytes<32>>>();

        let _event = BurnTokenEvent::decode_raw_log(topics, &raw.data.to_vec(), true).unwrap();
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
                    address: ethers_core::types::H160::default(),
                    topics: vec![],
                    data: ethers_core::types::Bytes::default(),
                    block_hash: None,
                    block_number: None,
                    transaction_hash: None,
                    transaction_index: None,
                    log_index: None,
                    transaction_log_index: None,
                    log_type: None,
                    removed: None,
                }],
            );
        }

        let client = FakeEthJsonRpcClient {
            logs,
            error: Some(802),
        };
        let evm_client = EthJsonRpcClient::new(client);

        // get from 0 to 100
        let logs =
            BridgeEvent::collect_logs(&evm_client, 0, 100, ethers_core::types::H160::default())
                .await
                .unwrap();
        assert_eq!(logs.len(), 0);

        // get from 80 to 220 (first result will be empty)
        let logs =
            BridgeEvent::collect_logs(&evm_client, 80, 220, ethers_core::types::H160::default())
                .await
                .unwrap();
        assert_eq!(logs.len(), 21);

        // get from 100 to 800 (multiple requests)
        let logs =
            BridgeEvent::collect_logs(&evm_client, 100, 800, ethers_core::types::H160::default())
                .await
                .unwrap();
        assert_eq!(logs.len(), 601);

        // get error block
        let logs =
            BridgeEvent::collect_logs(&evm_client, 801, 950, ethers_core::types::H160::default())
                .await
                .unwrap();
        assert_eq!(logs.len(), 950 - 801); // error will be skipped

        // get with more blocks than available
        let logs =
            BridgeEvent::collect_logs(&evm_client, 10, 2000, ethers_core::types::H160::default())
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
