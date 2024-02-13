use candid::CandidType;
use ethers_core::abi::{
    Constructor, Event, EventParam, Function, Param, ParamType, RawLog, StateMutability, Token,
};
use ethers_core::types::{H160, U256};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

pub static CONSTRUCTOR: Lazy<Constructor> = Lazy::new(|| Constructor {
    inputs: vec![Param {
        name: "minterAddress".into(),
        kind: ParamType::Address,
        internal_type: None,
    }],
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
            kind: ParamType::FixedBytes(32),
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
pub static FINISH_BURN: Lazy<Function> = Lazy::new(|| Function {
    name: "finishBurn".into(),
    inputs: vec![Param {
        name: "operationID".into(),
        kind: ParamType::Uint(32),
        internal_type: None,
    }],
    outputs: vec![Param {
        name: "".into(),
        kind: ParamType::Bool,
        internal_type: None,
    }],
    constant: None,
    state_mutability: StateMutability::NonPayable,
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
            kind: ParamType::FixedBytes(32),
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

#[derive(Debug, Default)]
struct BurntEventDataBuilder {
    pub sender: Option<H160>,
    pub amount: Option<U256>,
    pub from_erc20: Option<H160>,
    pub recipient_id: Option<Vec<u8>>,
    pub to_token: Option<Vec<u8>>,
    operation_id: Option<u32>,
    pub name: Option<Vec<u8>>,
    pub symbol: Option<Vec<u8>>,
    pub decimals: Option<u8>,
}

impl BurntEventDataBuilder {
    fn build(self) -> Result<BurntEventData, ethers_core::abi::Error> {
        fn not_found(field: &str) -> impl FnOnce() -> ethers_core::abi::Error {
            let msg = format!("missing event field `{}`", field);
            move || ethers_core::abi::Error::Other(msg.into())
        }

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

    fn with_field_from_token(&mut self, name: &str, value: Token) -> &mut Self {
        match name {
            "sender" => self.sender = value.into_address(),
            "amount" => self.amount = value.into_uint(),
            "fromERC20" => self.from_erc20 = value.into_address(),
            "recipientID" => self.recipient_id = value.into_fixed_bytes(),
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
            data_builder.with_field_from_token(&param.name, param.value);
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

#[derive(Debug, Default, Clone, CandidType, Serialize, Deserialize)]
pub struct MintedEventData {
    pub amount: did::U256,
    pub from_token: Vec<u8>,
    pub sender_id: Vec<u8>,
    pub to_erc20: did::H160,
    pub recipient: did::H160,
    pub nonce: u32,
}

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
    fn build(self) -> Result<MintedEventData, ethers_core::abi::Error> {
        fn not_found(field: &str) -> impl FnOnce() -> ethers_core::abi::Error {
            let msg = format!("missing event field `{}`", field);
            move || ethers_core::abi::Error::Other(msg.into())
        }

        Ok(MintedEventData {
            amount: self.amount.ok_or_else(not_found("amount"))?.into(),
            from_token: self.from_token.ok_or_else(not_found("fromToken"))?,
            sender_id: self.sender_id.ok_or_else(not_found("senderID"))?,
            to_erc20: self.to_erc20.ok_or_else(not_found("toERC20"))?.into(),
            recipient: self.recipient.ok_or_else(not_found("recipient"))?.into(),
            nonce: self.nonce.ok_or_else(not_found("nonce"))?,
        })
    }

    fn with_field_from_token(&mut self, name: &str, value: Token) -> &mut Self {
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
            data_builder.with_field_from_token(&param.name, param.value);
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
