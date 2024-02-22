use did::H160;
use eth_signer::{Signer, Wallet};
use ethers_core::abi::Token;
use ethers_core::k256::ecdsa::SigningKey;
use minter_contract_utils::build_data::test_contracts::{
    UNISWAP_FACTORY_HEX_CODE, UNISWAP_TOKEN_HEX_CODE,
};
use minter_contract_utils::uniswap_api::{
    UNISWAP_FACTORY_CONSTRUCTOR, UNISWAP_FACTORY_CREATE_PAIR, UNISWAP_TOKEN_CONSTRUCTOR,
};

use super::PocketIcTestContext;
use crate::context::{CanisterType, TestContext};

/// Uniswap environment for tests.
struct UniswapEnv {
    pub factory: H160,
    pub token0: H160,
    pub token1: H160,
    pub pair: H160,
}

impl UniswapEnv {
    /// Creates an Uniswap environment with new contracts.
    pub async fn new(ctx: &PocketIcTestContext, owner: &Wallet<'_, SigningKey>) -> Self {
        let factory = Self::create_factory(ctx, owner).await;
        let token0 = Self::create_token(ctx, owner).await;
        let token1 = Self::create_token(ctx, owner).await;
        let pair =
            Self::create_pair(ctx, factory.clone(), token0.clone(), token1.clone(), owner).await;

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
            .encode_input(contract, &[Token::Address(owner.address())])
            .unwrap();
        ctx.create_contract(owner, input).await.unwrap()
    }

    async fn create_token(ctx: &PocketIcTestContext, owner: &Wallet<'_, SigningKey>) -> H160 {
        let contract = UNISWAP_TOKEN_HEX_CODE.clone();
        let input = UNISWAP_TOKEN_CONSTRUCTOR
            .encode_input(contract, &[])
            .unwrap();
        ctx.create_contract(owner, input).await.unwrap()
    }

    async fn create_pair(
        ctx: &PocketIcTestContext,
        factory: H160,
        token0: H160,
        token1: H160,
        owner: &Wallet<'_, SigningKey>,
    ) -> H160 {
        let input = UNISWAP_FACTORY_CREATE_PAIR
            .encode_input(&[Token::Address(token0.into()), Token::Address(token1.into())])
            .unwrap();
        let output = ctx
            .call_contract(owner, &factory, input, 0)
            .await
            .unwrap()
            .1;
        let decoded = UNISWAP_FACTORY_CREATE_PAIR.decode_output(&output.output.unwrap());
        decoded.unwrap()[0].clone().into_address().unwrap().into()
    }
}

#[tokio::test]
async fn should_initialize_uniswap_env() {
    let ctx = PocketIcTestContext::new(&CanisterType::EVM_TEST_SET).await;

    let owner = ctx.new_wallet(u128::MAX).await.unwrap();

    let uniswap_env = UniswapEnv::new(&ctx, &owner).await;

    assert!(uniswap_env.factory != H160::zero());
    assert!(uniswap_env.token0 != H160::zero());
    assert!(uniswap_env.token1 != H160::zero());
    assert!(uniswap_env.pair != H160::zero());
}
