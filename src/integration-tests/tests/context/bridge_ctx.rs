use std::collections::HashMap;

use bridge_did::id256::Id256;
use did::{H160, U256};

use crate::context::TestContext;
use crate::utils::error::Result;

pub struct BridgeTestConfig {
    pub base_tokens_number: usize,
    pub users_number: usize,
    pub operations_per_user: usize,
}

pub struct BridgeTestState<Ctx: BridgeTestContext> {
    ctx: Ctx,
    users: Vec<Ctx::UserId>,
}

pub struct TokenPair<BaseId> {
    pub base: BaseId,
    pub wrapped: H160,
}

pub struct BridgeContracts {
    pub bft_bridge_contract: H160,
    pub fee_charge_contract: H160,
}

impl BridgeContracts {
    pub async fn init(ctx: &impl TestContext) -> Result<Self> {
        todo!()
    }
}

pub trait BridgeTestContext: TestContext {
    type UserId;
    type TokenId;

    async fn new_base_token(&self) -> Result<Self::TokenId>;
    async fn new_user(&self) -> Result<Self::UserId>;
    async fn mint_base_token(&self, user: &Self::UserId, token: &Self::TokenId);
    async fn deposit(
        &self,
        bridge: H160,
        from: &Self::UserId,
        to: H160,
        amount: U256,
    ) -> Result<U256>;

    async fn withdraw(
        &self,
        bridge: H160,
        from: &Self::UserId,
        to: H160,
        amount: U256,
    ) -> Result<U256>;
}
