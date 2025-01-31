use std::sync::Arc;

use candid::{Nat, Principal};
use ic_exports::icrc_types::icrc1::transfer::TransferArg;

use super::PocketIcTestContext;
use crate::context::TestContext;
use crate::pocket_ic_integration_test::{CanisterType, ADMIN};
use crate::utils::GanacheEvm;

#[tokio::test]
async fn test_transfer_tokens() {
    let ctx = PocketIcTestContext::new(
        &[CanisterType::Token1],
        Arc::new(GanacheEvm::run().await),
        Arc::new(GanacheEvm::run().await),
    )
    .await;
    let client = ctx.icrc_token_1_client(ADMIN);
    let amount = Nat::from(100_u64);
    let to = Principal::anonymous().into();

    let transfer_arg = TransferArg {
        from_subaccount: None,
        to,
        fee: None,
        created_at_time: None,
        memo: None,
        amount: amount.clone(),
    };

    client.icrc1_transfer(transfer_arg).await.unwrap().unwrap();
    let balance = client.icrc1_balance_of(to).await.unwrap();

    assert_eq!(balance, amount);
}
