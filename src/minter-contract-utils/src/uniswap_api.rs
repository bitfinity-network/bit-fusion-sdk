use ethers_core::abi::{Constructor, Function, Param, ParamType, StateMutability};
use once_cell::sync::Lazy;

pub static UNISWAP_TOKEN_CONSTRUCTOR: Lazy<Constructor> =
    Lazy::new(|| Constructor { inputs: vec![] });

pub static UNISWAP_FACTORY_CONSTRUCTOR: Lazy<Constructor> = Lazy::new(|| Constructor {
    inputs: vec![Param {
        name: "_feeToSetter".into(),
        kind: ParamType::Address,
        internal_type: None,
    }],
});

#[allow(deprecated)] // need to initialize `constant` field
pub static UNISWAP_FACTORY_CREATE_PAIR: Lazy<Function> = Lazy::new(|| Function {
    name: "createPair".into(),
    inputs: vec![
        Param {
            name: "tokenA".into(),
            kind: ParamType::Address,
            internal_type: None,
        },
        Param {
            name: "tokenB".into(),
            kind: ParamType::Address,
            internal_type: None,
        },
    ],
    outputs: vec![Param {
        name: "pair".into(),
        kind: ParamType::Address,
        internal_type: None,
    }],
    constant: None,
    state_mutability: StateMutability::NonPayable,
});
