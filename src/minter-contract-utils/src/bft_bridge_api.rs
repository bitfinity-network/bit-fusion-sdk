use candid::CandidType;
use ethereum_json_rpc_client::{Client, EthGetLogsParams, EthJsonRpcClient};
use ethers_core::abi::{
    Constructor, Event, EventParam, Function, Param, ParamType, RawLog, StateMutability, Token,
};
use ethers_core::types::{BlockNumber as EthBlockNumber, Log, Transaction, H160, U256};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

pub static CONSTRUCTOR: Lazy<Constructor> = Lazy::new(|| Constructor {
    inputs: vec![
        Param {
            name: "minterAddress".into(),
            kind: ParamType::Address,
            internal_type: None,
        },
        Param {
            name: "feeChargeAddress".into(),
            kind: ParamType::Address,
            internal_type: None,
        },
    ],
});

#[allow(deprecated)] // need to initialize `constant` field
pub static MINTER_CANISTER_ADDRESS: Lazy<Function> = Lazy::new(|| Function {
    name: "minterCanisterAddress".into(),
    inputs: vec![],
    outputs: vec![Param {
        name: "".into(),
        kind: ParamType::Address,
        internal_type: None,
    }],
    constant: None,
    state_mutability: StateMutability::View,
});

#[allow(deprecated)] // need to initialize `constant` field
pub static BURN: Lazy<Function> = Lazy::new(|| Function {
    name: "burn".into(),
    inputs: vec![
        Param {
            name: "amount".into(),
            kind: ParamType::Uint(256),
            internal_type: None,
        },
        Param {
            name: "fromERC20".into(),
            kind: ParamType::Address,
            internal_type: None,
        },
        Param {
            name: "recipientID".into(),
            kind: ParamType::Bytes,
            internal_type: None,
        },
    ],
    outputs: vec![Param {
        name: "".into(),
        kind: ParamType::Uint(32),
        internal_type: None,
    }],
    constant: None,
    state_mutability: StateMutability::NonPayable,
});

pub fn decode_burn_operation_id(raw_data: &[u8]) -> anyhow::Result<u32> {
    let id = BURN
        .decode_output(raw_data)?
        .first()
        .cloned()
        .ok_or_else(|| anyhow::Error::msg("no tokens in burn operation output"))?
        .into_uint()
        .ok_or_else(|| anyhow::Error::msg("wrong token in burn operation output"))?
        .as_u32();
    Ok(id)
}

#[allow(deprecated)] // need to initialize `constant` field
pub static GET_PENDING_BURN_INFO: Lazy<Function> = Lazy::new(|| Function {
    name: "getPendingBurnInfo".into(),
    inputs: vec![
        Param {
            name: "user".into(),
            kind: ParamType::Address,
            internal_type: None,
        },
        Param {
            name: "operationID".into(),
            kind: ParamType::Uint(32),
            internal_type: None,
        },
    ],
    outputs: vec![
        Param {
            name: "sender".into(),
            kind: ParamType::Address,
            internal_type: None,
        },
        Param {
            name: "amount".into(),
            kind: ParamType::Uint(256),
            internal_type: None,
        },
        Param {
            name: "fromERC20".into(),
            kind: ParamType::Address,
            internal_type: None,
        },
        Param {
            name: "recipientID".into(),
            kind: ParamType::FixedBytes(32),
            internal_type: None,
        },
        Param {
            name: "toToken".into(),
            kind: ParamType::FixedBytes(32),
            internal_type: None,
        },
        Param {
            name: "name".into(),
            kind: ParamType::FixedBytes(32),
            internal_type: None,
        },
        Param {
            name: "symbol".into(),
            kind: ParamType::FixedBytes(16),
            internal_type: None,
        },
        Param {
            name: "decimals".into(),
            kind: ParamType::Uint(8),
            internal_type: None,
        },
    ],
    constant: None,
    state_mutability: StateMutability::View,
});

#[allow(deprecated)] // need to initialize `constant` field
pub static MINT: Lazy<Function> = Lazy::new(|| Function {
    name: "mint".into(),
    inputs: vec![Param {
        name: "encodedOrder".into(),
        kind: ParamType::Bytes,
        internal_type: None,
    }],
    outputs: vec![],
    constant: None,
    state_mutability: StateMutability::NonPayable,
});

#[allow(deprecated)] // need to initialize `constant` field
pub static DEPLOY_WRAPPED_TOKEN: Lazy<Function> = Lazy::new(|| Function {
    name: "deployERC20".into(),
    inputs: vec![
        Param {
            name: "name".into(),
            kind: ParamType::String,
            internal_type: None,
        },
        Param {
            name: "symbol".into(),
            kind: ParamType::String,
            internal_type: None,
        },
        Param {
            name: "baseTokenID".into(),
            kind: ParamType::FixedBytes(32),
            internal_type: None,
        },
    ],
    outputs: vec![Param {
        name: "".into(),
        kind: ParamType::Address,
        internal_type: None,
    }],
    constant: None,
    state_mutability: StateMutability::NonPayable,
});

pub static BURNT_EVENT: Lazy<Event> = Lazy::new(|| Event {
    name: "BurnTokenEvent".into(),
    inputs: vec![
        EventParam {
            name: "sender".into(),
            kind: ParamType::Address,
            indexed: false,
        },
        EventParam {
            name: "amount".into(),
            kind: ParamType::Uint(256),
            indexed: false,
        },
        EventParam {
            name: "fromERC20".into(),
            kind: ParamType::Address,
            indexed: false,
        },
        EventParam {
            name: "recipientID".into(),
            kind: ParamType::Bytes,
            indexed: false,
        },
        EventParam {
            name: "toToken".into(),
            kind: ParamType::FixedBytes(32),
            indexed: false,
        },
        EventParam {
            name: "operationID".into(),
            kind: ParamType::Uint(32),
            indexed: false,
        },
        EventParam {
            name: "name".into(),
            kind: ParamType::FixedBytes(32),
            indexed: false,
        },
        EventParam {
            name: "symbol".into(),
            kind: ParamType::FixedBytes(16),
            indexed: false,
        },
        EventParam {
            name: "decimals".into(),
            kind: ParamType::Uint(8),
            indexed: false,
        },
    ],
    anonymous: false,
});

/// Emited when token is burnt or minted by BFTBridge.
#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub enum BridgeEvent {
    Burnt(BurntEventData),
    Minted(MintedEventData),
    Notify(NotifyMinterEventData),
}

impl BridgeEvent {
    pub async fn collect_logs(
        evm_client: &EthJsonRpcClient<impl Client>,
        from_block: EthBlockNumber,
        to_block: EthBlockNumber,
        bridge_contract: H160,
    ) -> Result<Vec<Log>, anyhow::Error> {
        let params = EthGetLogsParams {
            address: Some(vec![bridge_contract]),
            from_block,
            to_block,
            topics: Some(vec![vec![
                BURNT_EVENT.signature(),
                MINTED_EVENT.signature(),
                NOTIFY_EVENT.signature(),
            ]]),
        };

        evm_client.get_logs(params).await
    }

    pub fn from_log(log: Log) -> Result<Self, ethers_core::abi::Error> {
        let raw_log = RawLog {
            topics: log.topics,
            data: log.data.to_vec(),
        };

        Self::try_from(raw_log)
    }
}

impl TryFrom<RawLog> for BridgeEvent {
    type Error = ethers_core::abi::Error;

    fn try_from(log: RawLog) -> Result<Self, Self::Error> {
        BurntEventData::try_from(log.clone())
            .map(Self::Burnt)
            .or_else(|_| MintedEventData::try_from(log.clone()).map(Self::Minted))
            .or_else(|_| NotifyMinterEventData::try_from(log).map(Self::Notify))
    }
}

/// Emited when token is burnt by BFTBridge.
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

fn not_found(field: &str) -> impl FnOnce() -> ethers_core::abi::Error {
    let msg = format!("missing event field `{}`", field);
    move || ethers_core::abi::Error::Other(msg.into())
}

/// Builds `BurntEventData` from tokens.
#[derive(Debug, Default)]
struct BurntEventDataBuilder {
    pub sender: Option<H160>,
    pub amount: Option<U256>,
    pub from_erc20: Option<H160>,
    pub recipient_id: Option<Vec<u8>>,
    pub to_token: Option<Vec<u8>>,
    pub operation_id: Option<u32>,
    pub name: Option<Vec<u8>>,
    pub symbol: Option<Vec<u8>>,
    pub decimals: Option<u8>,
}

impl BurntEventDataBuilder {
    /// Builds `BurntEventData` from tokens.
    /// All fields are required.
    fn build(self) -> Result<BurntEventData, ethers_core::abi::Error> {
        Ok(BurntEventData {
            sender: self.sender.ok_or_else(not_found("sender"))?.into(),
            amount: self.amount.ok_or_else(not_found("amount"))?.into(),
            from_erc20: self.from_erc20.ok_or_else(not_found("fromERC20"))?.into(),
            recipient_id: self.recipient_id.ok_or_else(not_found("recipientID"))?,
            to_token: self.to_token.ok_or_else(not_found("toToken"))?,
            operation_id: self.operation_id.ok_or_else(not_found("operationID"))?,
            name: self.name.ok_or_else(not_found("name"))?,
            symbol: self.symbol.ok_or_else(not_found("symbol"))?,
            decimals: self.decimals.ok_or_else(not_found("decimals"))?,
        })
    }

    fn with_field_from_token(mut self, name: &str, value: Token) -> Self {
        match name {
            "sender" => self.sender = value.into_address(),
            "amount" => self.amount = value.into_uint(),
            "fromERC20" => self.from_erc20 = value.into_address(),
            "recipientID" => self.recipient_id = value.into_bytes(),
            "toToken" => self.to_token = value.into_fixed_bytes(),
            "operationID" => self.operation_id = value.into_uint().map(|v| v.as_u32()),
            "name" => self.name = value.into_fixed_bytes(),
            "symbol" => self.symbol = value.into_fixed_bytes(),
            "decimals" => self.decimals = value.into_uint().map(|v| v.as_u32() as _),
            _ => {}
        };
        self
    }
}

impl TryFrom<RawLog> for BurntEventData {
    type Error = ethers_core::abi::Error;

    fn try_from(log: RawLog) -> Result<Self, Self::Error> {
        let parsed = BURNT_EVENT.parse_log(log)?;

        let mut data_builder = BurntEventDataBuilder::default();

        for param in parsed.params {
            data_builder = data_builder.with_field_from_token(&param.name, param.value);
        }

        data_builder.build()
    }
}

pub static MINTED_EVENT: Lazy<Event> = Lazy::new(|| Event {
    name: "MintTokenEvent".into(),
    inputs: vec![
        EventParam {
            name: "amount".into(),
            kind: ParamType::Uint(256),
            indexed: false,
        },
        EventParam {
            name: "fromToken".into(),
            kind: ParamType::FixedBytes(32),
            indexed: false,
        },
        EventParam {
            name: "senderID".into(),
            kind: ParamType::FixedBytes(32),
            indexed: false,
        },
        EventParam {
            name: "toERC20".into(),
            kind: ParamType::Address,
            indexed: false,
        },
        EventParam {
            name: "recipient".into(),
            kind: ParamType::Address,
            indexed: false,
        },
        EventParam {
            name: "nonce".into(),
            kind: ParamType::Uint(32),
            indexed: false,
        },
    ],
    anonymous: false,
});

#[allow(deprecated)] // need to initialize `constant` field
pub static NOTIFY_MINTER: Lazy<Function> = Lazy::new(|| Function {
    name: "notifyMinter".to_string(),
    inputs: vec![
        Param {
            name: "notificationType".into(),
            kind: ParamType::Uint(32),
            internal_type: None,
        },
        Param {
            name: "userData".into(),
            kind: ParamType::Bytes,
            internal_type: None,
        },
    ],
    outputs: vec![],
    constant: None,
    state_mutability: StateMutability::NonPayable,
});

pub static NOTIFY_EVENT: Lazy<Event> = Lazy::new(|| Event {
    name: "NotifyMinterEvent".into(),
    inputs: vec![
        EventParam {
            name: "notificationType".into(),
            kind: ParamType::Uint(32),
            indexed: false,
        },
        EventParam {
            name: "userData".into(),
            kind: ParamType::Bytes,
            indexed: false,
        },
    ],
    anonymous: false,
});

/// Event emitted when token is minted by BFTBridge.
#[derive(Debug, Default, Clone, CandidType, Serialize, Deserialize)]
pub struct MintedEventData {
    pub amount: did::U256,
    pub from_token: Vec<u8>,
    pub sender_id: Vec<u8>,
    pub to_erc20: did::H160,
    pub recipient: did::H160,
    pub nonce: u32,
}

/// Builds `MintedEventData` from tokens.
#[derive(Debug, Default)]
struct MintedEventDataBuilder {
    pub amount: Option<U256>,
    pub from_token: Option<Vec<u8>>,
    pub sender_id: Option<Vec<u8>>,
    pub to_erc20: Option<H160>,
    pub recipient: Option<H160>,
    pub nonce: Option<u32>,
}

impl MintedEventDataBuilder {
    /// Builds `MintedEventData` from tokens.
    /// All fields are required.
    fn build(self) -> Result<MintedEventData, ethers_core::abi::Error> {
        Ok(MintedEventData {
            amount: self.amount.ok_or_else(not_found("amount"))?.into(),
            from_token: self.from_token.ok_or_else(not_found("fromToken"))?,
            sender_id: self.sender_id.ok_or_else(not_found("senderID"))?,
            to_erc20: self.to_erc20.ok_or_else(not_found("toERC20"))?.into(),
            recipient: self.recipient.ok_or_else(not_found("recipient"))?.into(),
            nonce: self.nonce.ok_or_else(not_found("nonce"))?,
        })
    }

    fn with_field_from_token(mut self, name: &str, value: Token) -> Self {
        match name {
            "amount" => self.amount = value.into_uint().map(Into::into),
            "fromToken" => self.from_token = value.into_fixed_bytes(),
            "senderID" => self.sender_id = value.into_fixed_bytes(),
            "toERC20" => self.to_erc20 = value.into_address().map(Into::into),
            "recipient" => self.recipient = value.into_address().map(Into::into),
            "nonce" => self.nonce = value.into_uint().map(|v| v.as_u32()),
            _ => {}
        };
        self
    }
}

impl TryFrom<RawLog> for MintedEventData {
    type Error = ethers_core::abi::Error;

    fn try_from(log: RawLog) -> Result<Self, Self::Error> {
        let parsed = MINTED_EVENT.parse_log(log)?;

        let mut data_builder = MintedEventDataBuilder::default();

        for param in parsed.params {
            data_builder = data_builder.with_field_from_token(&param.name, param.value);
        }

        data_builder.build()
    }
}

#[derive(Debug, PartialEq, Eq, Clone, CandidType, Serialize, Deserialize)]
pub struct NotifyMinterEventData {
    pub notification_type: u32,
    pub user_data: Vec<u8>,
}

#[derive(Debug, Default, Clone)]
struct NotifyMinterEventDataBuilder {
    notification_type: Option<u32>,
    user_data: Option<Vec<u8>>,
}

impl NotifyMinterEventDataBuilder {
    fn build(self) -> Result<NotifyMinterEventData, ethers_core::abi::Error> {
        Ok(NotifyMinterEventData {
            notification_type: self
                .notification_type
                .ok_or_else(not_found("notificationType"))?,
            user_data: self.user_data.ok_or_else(not_found("userData"))?,
        })
    }

    fn with_field_from_token(mut self, name: &str, value: Token) -> Self {
        match name {
            "notificationType" => self.notification_type = value.into_uint().map(|v| v.as_u32()),
            "userData" => self.user_data = value.into_bytes().map(Into::into),
            _ => {}
        };
        self
    }
}

impl TryFrom<RawLog> for NotifyMinterEventData {
    type Error = ethers_core::abi::Error;

    fn try_from(log: RawLog) -> Result<Self, Self::Error> {
        let parsed = NOTIFY_EVENT.parse_log(log)?;

        let mut data_builder = NotifyMinterEventDataBuilder::default();

        for param in parsed.params {
            data_builder = data_builder.with_field_from_token(&param.name, param.value);
        }

        data_builder.build()
    }
}

#[allow(deprecated)] // need to initialize `constant` field
pub static GET_WRAPPED_TOKEN: Lazy<Function> = Lazy::new(|| Function {
    name: "getWrappedToken".into(),
    inputs: vec![Param {
        name: "baseTokenID".into(),
        kind: ParamType::FixedBytes(32),
        internal_type: None,
    }],
    outputs: vec![Param {
        name: "".into(),
        kind: ParamType::Address,
        internal_type: None,
    }],
    constant: None,
    state_mutability: StateMutability::View,
});

#[allow(deprecated)] // need to initialize `constant` field
pub static LIST_TOKEN_PAIRS: Lazy<Function> = Lazy::new(|| Function {
    name: "listTokenPairs".into(),
    inputs: vec![],
    outputs: vec![
        Param {
            name: "wrapped".into(),
            kind: ParamType::Array(Box::new(ParamType::Address)),
            internal_type: None,
        },
        Param {
            name: "base".into(),
            kind: ParamType::Array(Box::new(ParamType::FixedBytes(32))),
            internal_type: None,
        },
    ],
    constant: None,
    state_mutability: StateMutability::View,
});

pub fn deploy_transaction(
    sender: H160,
    nonce: U256,
    gas_price: U256,
    chain_id: u32,
    code: Vec<u8>,
    minter_address: H160,
    fee_charge_address: H160,
) -> Transaction {
    let data = CONSTRUCTOR
        .encode_input(
            code,
            &[
                Token::Address(minter_address),
                Token::Address(fee_charge_address),
            ],
        )
        .expect("constructor parameters encoding should pass");

    pub const DEFAULT_TX_GAS_LIMIT: u64 = 5_000_000;
    ethers_core::types::Transaction {
        from: sender,
        nonce,
        value: U256::zero(),
        gas: DEFAULT_TX_GAS_LIMIT.into(),
        gas_price: Some(gas_price),
        input: data.into(),
        chain_id: Some(chain_id.into()),
        ..Default::default()
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
    let data = MINT
        .encode_input(&[Token::Bytes(mint_order_data.to_vec())])
        .expect("mint order encoding should pass");

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
    use did::{H160, H256};
    use ethers_core::abi::{Bytes, RawLog, Token};
    use ethers_core::utils::hex::traits::FromHex;

    use crate::bft_bridge_api::{BurntEventDataBuilder, MintedEventDataBuilder};

    use super::{BurntEventData, MintedEventData};

    #[test]
    fn minted_event_data_builder_test() {
        let amount = 42.into();
        let from_token = vec![1; 32];
        let sender_id = vec![2; 32];
        let to_erc20 = H160::from_slice(&[3; 20]);
        let recipient = H160::from_slice(&[4; 20]);
        let nonce = 42u64.into();

        let event = MintedEventDataBuilder::default()
            .with_field_from_token("amount", Token::Uint(amount))
            .with_field_from_token("fromToken", Token::FixedBytes(from_token.clone()))
            .with_field_from_token("senderID", Token::FixedBytes(sender_id.clone()))
            .with_field_from_token("toERC20", Token::Address(to_erc20.0))
            .with_field_from_token("recipient", Token::Address(recipient.0))
            .with_field_from_token("nonce", Token::Uint(nonce))
            .build()
            .unwrap();

        assert_eq!(event.amount.0, amount);
        assert_eq!(event.from_token, from_token);
        assert_eq!(event.sender_id, sender_id);
        assert_eq!(event.to_erc20, to_erc20);
        assert_eq!(event.recipient, recipient);
        assert_eq!(event.nonce, nonce.as_u32());
    }

    #[test]
    fn burnt_event_data_builder_test() {
        let sender = H160::from_slice(&[3; 20]);
        let amount = 42.into();
        let from_erc20 = H160::from_slice(&[3; 20]);
        let recipient_id = vec![2; 32];
        let to_token = vec![3; 32];
        let operation_id = 24.into();
        let name = vec![4; 32];
        let symbol = vec![5; 32];
        let decimals = 6u8.into();

        let event = BurntEventDataBuilder::default()
            .with_field_from_token("sender", Token::Address(sender.0))
            .with_field_from_token("amount", Token::Uint(amount))
            .with_field_from_token("fromERC20", Token::Address(from_erc20.0))
            .with_field_from_token("recipientID", Token::Bytes(recipient_id.clone()))
            .with_field_from_token("toToken", Token::FixedBytes(to_token.clone()))
            .with_field_from_token("operationID", Token::Uint(operation_id))
            .with_field_from_token("name", Token::FixedBytes(name.clone()))
            .with_field_from_token("symbol", Token::FixedBytes(symbol.clone()))
            .with_field_from_token("decimals", Token::Uint(decimals))
            .build()
            .unwrap();

        assert_eq!(event.sender, sender);
        assert_eq!(event.amount.0, amount);
        assert_eq!(event.from_erc20, from_erc20);
        assert_eq!(event.recipient_id, recipient_id);
        assert_eq!(event.to_token, to_token);
        assert_eq!(event.operation_id, operation_id.as_u32());
        assert_eq!(event.name, name);
        assert_eq!(event.symbol, symbol);
        assert_eq!(event.decimals, decimals.as_u32() as u8);
    }

    #[test]
    fn convert_raw_log_into_minted_event() {
        let raw = RawLog {
            topics: vec![H256::from_hex_str("0x4e37fc8684e0f7ad6a6c1178855450294a16b418314493bd7883699e6b3a0140").unwrap().0],
            data: Bytes::from_hex("0x00000000000000000000000000000000000000000000000000000000000003e80100056b29e76e9f3b04252ff67c2e623a34dd275f46e5b79f000000000000000100056b29a2d1f5f7d0d6e524a73194b76469eba08460ba4400000000000000000000000000000000000000119544f158a75a60beb83d3a44dd16100ad6d1e50000000000000000000000001e368dfb3f4a2d4e326d2111e6415ce54e7403250000000000000000000000000000000000000000000000000000000000000000").unwrap(),
        };

        let _event = MintedEventData::try_from(raw).unwrap();
    }

    #[test]
    fn convert_raw_log_into_burnt_event() {
        let raw = RawLog {
            topics: vec![H256::from_hex_str("0xfa3804fd5313cc219c6d3a833f7dbc2b1b48ac5edbae532006f1aa876a23eb79").unwrap().0],
            data: Bytes::from_hex("0x000000000000000000000000e41b09c6e9eaa79356b10f4181564b4bdb169d3500000000000000000000000000000000000000000000000000000000000003e80000000000000000000000002ea5d83d5a08d8556f726d3004a50aa8aa81c5c200000000000000000000000000000000000000000000000000000000000001200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000057617465726d656c6f6e0000000000000000000000000000000000000000000057544d0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200100056b29dc8b8e5954eebac85b3145745362adfa50d8ad9e00000000000000").unwrap(),
        };

        let _event = BurntEventData::try_from(raw).unwrap();
    }
}
