use ethers_core::abi::{Constructor, Function, Param, ParamType, StateMutability};
use once_cell::sync::Lazy;

pub static CONSTRUCTOR: Lazy<Constructor> = Lazy::new(|| Constructor {
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
            name: "owner".into(),
            kind: ParamType::Address,
            internal_type: None,
        },
    ],
});

#[allow(deprecated)] // need to initialize `constant` field
pub static TRANSFER: Lazy<Function> = Lazy::new(|| Function {
    name: "transfer".into(),
    inputs: vec![
        Param {
            name: "to".into(),
            kind: ParamType::Address,
            internal_type: None,
        },
        Param {
            name: "amount".into(),
            kind: ParamType::Uint(256),
            internal_type: None,
        },
    ],
    outputs: vec![Param {
        name: "".to_string(),
        kind: ParamType::Bool,
        internal_type: None,
    }],
    constant: None,
    state_mutability: StateMutability::NonPayable,
});

#[allow(deprecated)] // need to initialize `constant` field
pub static ERC_20_APPROVE: Lazy<Function> = Lazy::new(|| Function {
    name: "approve".into(),
    inputs: vec![
        Param {
            name: "spender".into(),
            kind: ParamType::Address,
            internal_type: None,
        },
        Param {
            name: "amount".into(),
            kind: ParamType::Uint(256),
            internal_type: None,
        },
    ],
    outputs: vec![Param {
        name: "".into(),
        kind: ParamType::Bool,
        internal_type: None,
    }],
    constant: None,
    state_mutability: StateMutability::NonPayable,
});

#[allow(deprecated)] // need to initialize `constant` field
pub static ERC_20_BALANCE: Lazy<Function> = Lazy::new(|| Function {
    name: "balanceOf".into(),
    inputs: vec![Param {
        name: "_owner".into(),
        kind: ParamType::Address,
        internal_type: None,
    }],
    outputs: vec![Param {
        name: "".into(),
        kind: ParamType::Uint(256),
        internal_type: None,
    }],
    constant: None,
    state_mutability: StateMutability::NonPayable,
});

#[allow(deprecated)] // need to initialize `constant` field
pub static ERC_20_ALLOWANCE: Lazy<Function> = Lazy::new(|| Function {
    name: "allowance".into(),
    inputs: vec![
        Param {
            name: "_owner".into(),
            kind: ParamType::Address,
            internal_type: None,
        },
        Param {
            name: "_spender".into(),
            kind: ParamType::Address,
            internal_type: None,
        },
    ],
    outputs: vec![Param {
        name: "remaining".into(),
        kind: ParamType::Uint(256),
        internal_type: None,
    }],
    constant: None,
    state_mutability: StateMutability::View,
});
