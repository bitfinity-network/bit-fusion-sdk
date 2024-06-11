use ethers_core::abi::{Constructor, Function, Param, ParamType, StateMutability};
use once_cell::sync::Lazy;

pub static CONSTRUCTOR: Lazy<Constructor> = Lazy::new(|| Constructor {
    inputs: vec![Param {
        name: "minterAddress".into(),
        kind: ParamType::Array(Box::new(ParamType::Address)),
        internal_type: None,
    }],
});

#[allow(deprecated)] // need to initialize `constant` field
pub static NATIVE_TOKEN_BALANCE: Lazy<Function> = Lazy::new(|| Function {
    name: "nativeTokenBalance".into(),
    inputs: vec![Param {
        name: "user".into(),
        kind: ParamType::Address,
        internal_type: None,
    }],
    outputs: vec![Param {
        name: "balance".into(),
        kind: ParamType::Uint(256),
        internal_type: None,
    }],
    constant: None,
    state_mutability: StateMutability::View,
});

#[allow(deprecated)] // need to initialize `constant` field
pub static NATIVE_TOKEN_DEPOSIT: Lazy<Function> = Lazy::new(|| Function {
    name: "nativeTokenDeposit".into(),
    inputs: vec![Param {
        name: "approvedSenderIDs".into(),
        kind: ParamType::Array(Box::new(ParamType::FixedBytes(32))),
        internal_type: None,
    }],
    outputs: vec![Param {
        name: "balance".into(),
        kind: ParamType::Uint(256),
        internal_type: None,
    }],
    constant: None,
    state_mutability: StateMutability::Payable,
});

#[allow(deprecated)] // need to initialize `constant` field
pub static REMOVE_APPROVED_SPENDER_IDS: Lazy<Function> = Lazy::new(|| Function {
    name: "removeApprovedSenderIDs".into(),
    inputs: vec![Param {
        name: "approvedSenderIDs".into(),
        kind: ParamType::Array(Box::new(ParamType::FixedBytes(32))),
        internal_type: None,
    }],
    outputs: vec![],
    constant: None,
    state_mutability: StateMutability::NonPayable,
});
