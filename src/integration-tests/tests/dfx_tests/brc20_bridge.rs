mod ctx;

use std::sync::Arc;
use std::time::Duration;

use bitcoin::Amount;
use ctx::{Brc20InitArgs, DEFAULT_MAX_AMOUNT, DEFAULT_MINT_AMOUNT, REQUIRED_CONFIRMATIONS};
use eth_signer::Signer;

use self::ctx::Brc20Context;
use crate::dfx_tests::block_until_succeeds;
use crate::utils::token_amount::TokenAmount;

/// Default deposit amount
const DEFAULT_DEPOSIT_AMOUNT: u128 = 10_000;
/// Default withdraw amount
const DEFAULT_WITHDRAW_AMOUNT: u128 = 3_000;
/// Default decimals
const DEFAULT_DECIMALS: u8 = 18;

#[tokio::test]
#[serial_test::serial]
async fn test_should_deposit_and_withdraw_brc20_tokens() {
    let deposit_amount = TokenAmount::from_int(DEFAULT_DEPOSIT_AMOUNT, DEFAULT_DECIMALS);
    let withdraw_amount = TokenAmount::from_int(DEFAULT_WITHDRAW_AMOUNT, DEFAULT_DECIMALS);
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

    // withdraw
    let brc20_balance = ctx
        .brc20_balance(ctx.brc20_wallet_address(), &brc20_tick)
        .await;

    ctx.send_btc(&deposit_address, Amount::from_sat(100_000_000)) // 1 BTC
        .await
        .expect("send btc failed");
    ctx.withdraw(&brc20_tick, withdraw_amount).await;

    ctx.mint_blocks(REQUIRED_CONFIRMATIONS).await;

    let ctx = Arc::new(ctx);
    let ctx_t = ctx.clone();

    let expected_brc20_balance = withdraw_amount + brc20_balance;
    let expected_erc20_balance = deposit_amount - withdraw_amount;
    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            Box::pin(async move {
                let new_brc20_balance = ctx
                .brc20_balance(ctx.brc20_wallet_address(), &brc20_tick)
                .await;
                if new_brc20_balance != expected_brc20_balance {
                    anyhow::bail!("Got BRC20 balance: {new_brc20_balance}; expected: {expected_brc20_balance}");
                }

                let new_erc20_balance = ctx.wrapped_balance(&brc20_tick, &ctx.eth_wallet).await;
                if new_erc20_balance != expected_erc20_balance.amount() {
                    anyhow::bail!("Got ERC20 balance: {new_erc20_balance}; expected: {}", expected_erc20_balance.amount());
                }

                Ok(())
            })
        }, Duration::from_secs(120)).await;

    ctx.stop().await;
}
