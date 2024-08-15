use std::collections::HashMap;

use did::H160;

use crate::context::TestContext;
use crate::utils::error::Result;

pub struct BridgeContextState<Ctx: BridgeTestContext> {
    ctx: Ctx,
    token_pairs: Vec<TokenPair<Ctx::BaseTokenId>>,
    users: HashMap<Ctx::BaseUserId, UserState>,
    bft_bridge: H160,
    fee_charge: H160,
}

pub struct UserState {}

pub struct TokenPair<BaseId> {
    base: BaseId,
    wrapped: H160,
}

pub trait BridgeTestContext: TestContext {
    type BaseTokenId;
    type BaseUserId;
}
