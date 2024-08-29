mod ctx;

use std::time::Duration;

use eth_signer::Signer;

use self::ctx::Brc20Context;
use crate::context::TestContext as _;

const DEFAULT_MAX_AMOUNT: u64 = 21_000_000;
const DEFAULT_MINT_AMOUNT: u64 = 100_000;

#[tokio::test]
async fn test_should_deposit_brc20_tokens() {
    let ctx = Brc20Context::new(&[ctx::generate_brc20_tick()]).await;
    let brc20_tick = ctx.brc20.brc20_tokens.iter().next().copied().unwrap();

    // Get initial balance
    ctx.mint_blocks(1).await;
    let brc20_balance = ctx
        .brc20_balance(ctx.brc20_wallet_address(), &brc20_tick)
        .await;
    assert_ne!(brc20_balance, 0);

    // deposit
    let wallet_address = ctx.eth_wallet.address().into();
    let deposit_address = ctx.get_deposit_address(&wallet_address).await;
    assert!(ctx
        .send_brc20(&deposit_address, brc20_tick, DEFAULT_MINT_AMOUNT)
        .await
        .is_ok());
    assert!(ctx
        .deposit(brc20_tick, DEFAULT_MINT_AMOUNT, &wallet_address)
        .await
        .is_ok());

    // advance
    ctx.inner.advance_time(Duration::from_secs(10)).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // check balance
    let new_brc20_balance = ctx
        .brc20_balance(ctx.brc20_wallet_address(), &brc20_tick)
        .await;
    assert_eq!(new_brc20_balance, brc20_balance - DEFAULT_MINT_AMOUNT);

    // check wrapped balance
    let updated_balance = ctx.wrapped_balance(&brc20_tick, &ctx.eth_wallet).await;
    assert_eq!(updated_balance, DEFAULT_MINT_AMOUNT as u128);

    // check canister balance
    let canister_balance = ctx.brc20_balance(&deposit_address, &brc20_tick).await;
    assert_eq!(canister_balance, DEFAULT_MINT_AMOUNT);

    ctx.stop().await;
}
