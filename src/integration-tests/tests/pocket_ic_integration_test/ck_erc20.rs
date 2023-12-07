use did::H160;
use eth_signer::{Wallet, Signer};
use ethers_core::abi::{Constructor, Param, ParamType, Token, Function};
use ethers_core::k256::ecdsa::SigningKey;
use minter_contract_utils::build_data::UNISWAP_FACTORY_HEX_CODE;
use once_cell::sync::Lazy;

use crate::context::TestContext;

use super::PocketIcTestContext;

struct UniswapEnv {
    factory: H160,
    token0: H160,
    token1: H160,
    pair: H160,
}

impl UniswapEnv {
    pub async fn new(ctx: &PocketIcTestContext, owner: &Wallet<'_, SigningKey>) -> Self {
        let factory = Self::create_factory(ctx, owner).await;
        let token0 = Self::create_token(ctx, "UNS0").await;
        let token1 = Self::create_token(ctx, "UNS1").await;
        let pair = Self::create_pair(ctx, factory.clone(), token0.clone(), token1.clone()).await;

        UniswapEnv {
            factory,
            token0,
            token1,
            pair,
        }
    }

    async fn create_factory(ctx: &PocketIcTestContext, owner: &Wallet<'_, SigningKey>) -> H160 {
        let contract = UNISWAP_FACTORY_HEX_CODE.clone();
        let input = UNISWAP_FACTORY_CONSTRUCTOR
            .encode_input(contract, &[Token::Address(owner.address().into())])
            .unwrap();
        ctx.create_contract(owner, input).await.unwrap()
    }

    async fn create_token(ctx: &PocketIcTestContext, arg: &str) -> H160 {
        todo!()
    }

    async fn create_pair(ctx: &PocketIcTestContext, factory: H160, token0: H160, token1: H160) -> H160 {
        todo!()
    }

}

pub static UNISWAP_FACTORY_CONSTRUCTOR: Lazy<Constructor> = Lazy::new(|| Constructor {
    inputs: vec![Param {
        name: "_feeToSetter".into(),
        kind: ParamType::Address,
        internal_type: None,
    }],
});

pub static UNISWAP_FACTORY_CREATE_PAIR: Lazy<Function> = Lazy::new(|| Function {
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