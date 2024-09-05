mod ctx;

use std::time::Duration;

use ctx::{Brc20InitArgs, DEFAULT_MAX_AMOUNT, DEFAULT_MINT_AMOUNT};
use eth_signer::Signer;

use self::ctx::Brc20Context;
use crate::context::TestContext as _;
use crate::utils::token_amount::TokenAmount;

/// Default deposit amount
const DEFAULT_DEPOSIT_AMOUNT: u128 = 10_000;
/// Default decimals
const DEFAULT_DECIMALS: u8 = 18;

#[tokio::test]
#[serial_test::serial]
async fn test_should_deposit_brc20_tokens() {
    let deposit_amount = TokenAmount::from_int(DEFAULT_DEPOSIT_AMOUNT, DEFAULT_DECIMALS);
    let brc20_tick = ctx::generate_brc20_tick();

    let ctx = Brc20Context::new(&[Brc20InitArgs {
        tick: brc20_tick,
        decimals: Some(DEFAULT_DECIMALS),
        limit: Some(DEFAULT_MINT_AMOUNT),
        max_supply: DEFAULT_MAX_AMOUNT,
    }])
    .await;

    // Get initial balance
    ctx.mint_blocks(1).await;
    let brc20_balance = ctx
        .brc20_balance(ctx.brc20_wallet_address(), &brc20_tick)
        .await;
    assert_ne!(brc20_balance.amount(), 0);
    println!("Initial balance: {}", brc20_balance);

    // deposit
    let wallet_address = ctx.eth_wallet.address().into();
    let deposit_address = ctx.get_deposit_address(&wallet_address).await;
    ctx.send_brc20(&deposit_address, brc20_tick, deposit_amount)
        .await
        .expect("send brc20 failed");
    ctx.deposit(brc20_tick, deposit_amount, &wallet_address)
        .await
        .expect("deposit failed");

    // advance
    ctx.inner.advance_time(Duration::from_secs(10)).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // check balance
    let new_brc20_balance = ctx
        .brc20_balance(ctx.brc20_wallet_address(), &brc20_tick)
        .await;
    assert_eq!(new_brc20_balance, brc20_balance - deposit_amount);

    // check wrapped balance
    let updated_balance = ctx.wrapped_balance(&brc20_tick, &ctx.eth_wallet).await;
    assert_eq!(updated_balance, deposit_amount.amount());

    // check canister balance
    let canister_balance = ctx.brc20_balance(&deposit_address, &brc20_tick).await;
    assert_eq!(canister_balance, deposit_amount);

    ctx.stop().await;
}
