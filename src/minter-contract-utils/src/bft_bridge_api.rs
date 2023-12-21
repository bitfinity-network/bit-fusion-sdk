use ethers_core::abi::{
    Constructor, Event, EventParam, Function, Param, ParamType, StateMutability,
};
use once_cell::sync::Lazy;

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
            kind: ParamType::FixedBytes(32),
            indexed: false,
        },
        EventParam {
            name: "recipient".into(),
            kind: ParamType::FixedBytes(32),
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
