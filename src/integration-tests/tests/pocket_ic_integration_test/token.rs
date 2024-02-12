use candid::{Nat, Principal};

use super::PocketIcTestContext;
use crate::context::TestContext;
use crate::pocket_ic_integration_test::{CanisterType, ADMIN};

#[tokio::test]
async fn test_transfer_tokens() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Token1]).await;
    let client = ctx.icrc_token_1_client(ADMIN);
    let amount = Nat::from(100_u64);
    let to = Principal::anonymous().into();

    client.icrc1_transfer(to, amount.clone()).await.unwrap();
    let balance = client.icrc1_balance_of(to).await.unwrap();

    assert_eq!(balance, amount);
}
