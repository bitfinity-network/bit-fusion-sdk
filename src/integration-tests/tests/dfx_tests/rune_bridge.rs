use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use bridge_did::runes::RuneName;
use did::{BlockNumber, H160};
use eth_signer::Signer as _;

use crate::context::rune_bridge::{
    generate_rune_name, RuneDepositStrategy, RunesContext, REQUIRED_CONFIRMATIONS,
};
use crate::context::TestContext;
use crate::dfx_tests::block_until_succeeds;

#[tokio::test]
async fn runes_bridging_flow() {
    let ctx = RunesContext::dfx(&[generate_rune_name()]).await;

    let rune_id = ctx.runes.runes.keys().next().copied().unwrap();
    // Mint one block in case there are some pending transactions
    ctx.wait_for_blocks(1).await;
    let ord_balance = ctx
        .ord_rune_balance(&ctx.runes.ord_wallet.address, &rune_id)
        .await
        .expect("failed to get ord balance");
    let wallet_address = ctx.eth_wallet.address().into();
    // get nonce
    let client = ctx.inner.evm_client(ctx.inner.admin_name());
    let nonce = client
        .eth_get_transaction_count(ctx.eth_wallet.address().into(), BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();

    ctx.deposit_runes_to(
        &[(&rune_id, 100)],
        &wallet_address,
        &ctx.eth_wallet,
        nonce,
        None,
        RuneDepositStrategy::AllInOne,
    )
    .await
    .expect("deposit failed");

    let recipient = ctx.runes.ord_wallet.address.clone();
    // withdraw back 30 of rune
    let ctx = Arc::new(ctx);
    let ctx_t = ctx.clone();
    block_until_succeeds(
        move || {
            let ctx_t = ctx_t.clone();
            let recipient = recipient.clone();

            Box::pin(async move { ctx_t.withdraw(&recipient, &rune_id, 30).await })
        },
        &ctx.inner,
        Duration::from_secs(60),
    )
    .await;

    ctx.wait_for_blocks(REQUIRED_CONFIRMATIONS).await;

    let updated_balance = ctx.wrapped_balance(&rune_id, &ctx.eth_wallet).await;
    assert_eq!(updated_balance, 70);

    let expected_balance = ord_balance - 100 + 30;

    let ctx_t = ctx.clone();
    // advance
    block_until_succeeds(
        move || {
            let ctx_t = ctx_t.clone();
            Box::pin(async move {
                let updated_ord_balance = ctx_t
                    .ord_rune_balance(&ctx_t.runes.ord_wallet.address, &rune_id)
                    .await?;
                if updated_ord_balance == expected_balance {
                    return Ok(());
                }

                Err(anyhow::anyhow!(
                    "Expected balance: {expected_balance}; got {updated_ord_balance}"
                ))
            })
        },
        &ctx.inner,
        Duration::from_secs(180),
    )
    .await;

    let updated_ord_balance = ctx
        .ord_rune_balance(&ctx.runes.ord_wallet.address, &rune_id)
        .await
        .expect("failed to get ord balance");

    assert_eq!(updated_ord_balance, expected_balance);

    ctx.stop().await
}

#[tokio::test]
async fn inputs_from_different_users() {
    let ctx = RunesContext::dfx(&[generate_rune_name()]).await;

    let rune_id = ctx.runes.runes.keys().next().copied().unwrap();
    // Mint one block in case there are some pending transactions
    ctx.wait_for_blocks(1).await;
    let rune_balance = ctx
        .ord_rune_balance(&ctx.runes.ord_wallet.address, &rune_id)
        .await
        .expect("failed to get ord balance");
    let wallet_address = ctx.eth_wallet.address().into();
    // get nonce
    let client = ctx.inner.evm_client(ctx.inner.admin_name());
    let nonce = client
        .eth_get_transaction_count(ctx.eth_wallet.address().into(), BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();
    ctx.deposit_runes_to(
        &[(&rune_id, 100)],
        &wallet_address,
        &ctx.eth_wallet,
        nonce,
        None,
        RuneDepositStrategy::AllInOne,
    )
    .await
    .expect("deposit failed");

    let another_wallet = ctx
        .inner
        .new_wallet(u128::MAX)
        .await
        .expect("failed to create an ETH wallet");
    let client = ctx.inner.evm_client(ctx.inner.admin_name());
    let nonce = client
        .eth_get_transaction_count(another_wallet.address().into(), BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();
    ctx.deposit_runes_to(
        &[(&rune_id, 77)],
        &another_wallet.address().into(),
        &another_wallet,
        nonce,
        None,
        RuneDepositStrategy::AllInOne,
    )
    .await
    .expect("deposit failed");

    //let recipient = ctx.get_deposit_address(&wallet_address).await;

    let ctx = Arc::new(ctx);
    ctx.withdraw(&ctx.runes.ord_wallet.address, &rune_id, 50)
        .await
        .expect("failed to withdraw");

    let ctx_t = ctx.clone();

    block_until_succeeds(
        move || {
            let ctx_t = ctx_t.clone();
            Box::pin(async move {
                let updated_balance = ctx_t.wrapped_balance(&rune_id, &ctx_t.eth_wallet).await;
                if updated_balance == 50 {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "Expected balance: 50; got {updated_balance}"
                    ))
                }
            })
        },
        &ctx.inner,
        Duration::from_secs(120),
    )
    .await;

    let expected_balance = rune_balance - 50 - 77;

    let ctx_t = ctx.clone();
    block_until_succeeds(
        move || {
            let ctx_t = ctx_t.clone();
            Box::pin(async move {
                let updated_rune_balance = ctx_t
                    .ord_rune_balance(&ctx_t.runes.ord_wallet.address, &rune_id)
                    .await?;
                println!("{} should be {}", updated_rune_balance, expected_balance);

                if updated_rune_balance == expected_balance {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "Expected balance: {expected_balance}; got {updated_rune_balance}"
                    ))
                }
            })
        },
        &ctx.inner,
        Duration::from_secs(120),
    )
    .await;

    assert_eq!(ctx.wrapped_balance(&rune_id, &another_wallet).await, 77);
    assert_eq!(ctx.wrapped_balance(&rune_id, &ctx.eth_wallet).await, 50);

    ctx.stop().await
}

#[tokio::test]
async fn test_should_deposit_two_runes_in_a_single_tx() {
    let ctx = RunesContext::dfx(&[generate_rune_name(), generate_rune_name()]).await;
    let foo_rune_id = ctx.runes.runes.keys().next().copied().unwrap();
    let bar_rune_id = ctx.runes.runes.keys().nth(1).copied().unwrap();

    // Mint one block in case there are some pending transactions
    ctx.wait_for_blocks(1).await;
    let before_balances = ctx
        .wrapped_balances(&[foo_rune_id, bar_rune_id], &ctx.eth_wallet)
        .await;
    let wallet_address = ctx.eth_wallet.address().into();
    // get nonce
    let client = ctx.inner.evm_client(ctx.inner.admin_name());
    let nonce = client
        .eth_get_transaction_count(ctx.eth_wallet.address().into(), BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();
    // deposit runes
    ctx.deposit_runes_to(
        &[(&foo_rune_id, 100), (&bar_rune_id, 200)],
        &wallet_address,
        &ctx.eth_wallet,
        nonce,
        None,
        RuneDepositStrategy::AllInOne,
    )
    .await
    .expect("deposit failed");

    // check balances
    let ctx = Arc::new(ctx);
    let ctx_t = ctx.clone();

    block_until_succeeds(
        move || {
            let ctx_t = ctx_t.clone();
            let before_balances = before_balances.clone();
            Box::pin(async move {
                let after_balances = ctx_t
                    .wrapped_balances(&[foo_rune_id, bar_rune_id], &ctx_t.eth_wallet)
                    .await;

                if after_balances[&foo_rune_id] == before_balances[&foo_rune_id] + 100
                    && after_balances[&bar_rune_id] == before_balances[&bar_rune_id] + 200
                {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Balances are not correct"))
                }
            })
        },
        &ctx.inner,
        Duration::from_secs(30),
    )
    .await;

    ctx.stop().await
}

#[tokio::test]
async fn test_should_deposit_two_runes_in_two_tx() {
    let ctx = RunesContext::dfx(&[generate_rune_name(), generate_rune_name()]).await;
    let foo_rune_id = ctx.runes.runes.keys().next().copied().unwrap();
    let bar_rune_id = ctx.runes.runes.keys().nth(1).copied().unwrap();

    // Mint one block in case there are some pending transactions
    ctx.wait_for_blocks(1).await;
    let before_balances = ctx
        .wrapped_balances(&[foo_rune_id, bar_rune_id], &ctx.eth_wallet)
        .await;
    let wallet_address = ctx.eth_wallet.address().into();
    // get nonce
    let client = ctx.inner.evm_client(ctx.inner.admin_name());
    let nonce = client
        .eth_get_transaction_count(ctx.eth_wallet.address().into(), BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();
    // deposit runes
    ctx.deposit_runes_to(
        &[(&foo_rune_id, 100), (&bar_rune_id, 200)],
        &wallet_address,
        &ctx.eth_wallet,
        nonce,
        None,
        RuneDepositStrategy::OnePerTx,
    )
    .await
    .expect("deposit failed");

    // check balances
    let ctx = Arc::new(ctx);
    let ctx_t = ctx.clone();

    block_until_succeeds(
        move || {
            let ctx_t = ctx_t.clone();
            let before_balances = before_balances.clone();
            Box::pin(async move {
                let after_balances = ctx_t
                    .wrapped_balances(&[foo_rune_id, bar_rune_id], &ctx_t.eth_wallet)
                    .await;

                if after_balances[&foo_rune_id] == before_balances[&foo_rune_id] + 100
                    && after_balances[&bar_rune_id] == before_balances[&bar_rune_id] + 200
                {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Balances are not correct"))
                }
            })
        },
        &ctx.inner,
        Duration::from_secs(30),
    )
    .await;

    ctx.stop().await
}

#[tokio::test]
async fn bail_out_of_impossible_deposit() {
    let rune_name = generate_rune_name();
    let ctx = RunesContext::dfx(&[rune_name.clone()]).await;

    let rune_id = ctx.runes.runes.keys().next().copied().unwrap();
    let rune_name = RuneName::from_str(&rune_name).unwrap();
    // Mint one block in case there are some pending transactions
    ctx.wait_for_blocks(1).await;
    let address = ctx
        .get_deposit_address(&ctx.eth_wallet.address().into())
        .await;
    // get nonce
    let client = ctx.inner.evm_client(ctx.inner.admin_name());
    let nonce = client
        .eth_get_transaction_count(ctx.eth_wallet.address().into(), BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();

    ctx.send_runes(&ctx.runes.ord_wallet, &address, &[(&rune_id, 10_000)])
        .await
        .expect("send runes failed");
    ctx.send_deposit_notification(
        &[rune_id],
        Some([(rune_name, 5000)].into()),
        &ctx.eth_wallet.address().into(),
        &ctx.eth_wallet,
        nonce,
        None,
    )
    .await;

    ctx.inner.advance_time(Duration::from_secs(10)).await;
    ctx.inner.advance_by_times(Duration::from_secs(5), 3).await;

    let client = std::sync::Arc::new(ctx.inner.rune_bridge_client(ctx.inner.admin_name()));
    let address = ctx.eth_wallet.address();

    let operations = block_until_succeeds(
        move || {
            let client = client.clone();
            Box::pin(async move {
                let operations = client
                    .get_operations_list(&address.into(), None, None)
                    .await?;

                if operations.len() == 1 {
                    Ok(operations)
                } else {
                    Err(anyhow::anyhow!(
                        "Expected 1 operation, got {}",
                        operations.len()
                    ))
                }
            })
        },
        &ctx.inner,
        Duration::from_secs(30),
    )
    .await;

    let client = ctx.inner.rune_bridge_client(ctx.inner.admin_name());
    let operation_id = operations[0].0;

    let log = client
        .get_operation_log(operation_id)
        .await
        .unwrap()
        .unwrap();

    let len = log.log().len();
    // First entry in the log is the scheduling of the operation, so we skip it. There might be other
    // errors, but none of them should be a `cannot progress` error, so we check it here.
    for entry in log.log().iter().take(len.saturating_sub(1)).skip(1) {
        assert!(!entry
            .step_result
            .clone()
            .unwrap_err()
            .to_string()
            .contains("operation cannot progress"));
    }

    assert!(log
        .log()
        .last()
        .unwrap()
        .step_result
        .clone()
        .unwrap_err()
        .to_string()
        .contains("operation cannot progress"));

    ctx.stop().await
}

#[tokio::test]
async fn generates_correct_deposit_address() {
    const ETH_ADDRESS: &str = "0x4e37fc8684e0f7ad6a6c1178855450294a16b418";
    let eth_address = H160::from_hex_str(ETH_ADDRESS).unwrap();

    let rune_name = generate_rune_name();
    let ctx = RunesContext::dfx(&[rune_name.clone()]).await;
    let address = ctx.get_deposit_address(&eth_address).await;

    assert_eq!(
        address.to_string(),
        "bcrt1quj4mrtx0grz3n2m3axjr65fhe67z8m836f674x"
    );

    const ANOTHER_ETH_ADDRESS: &str = "0x4e37fc8684e0f7ad6a6c1178855450294a16b419";
    let eth_address = H160::from_hex_str(ANOTHER_ETH_ADDRESS).unwrap();

    let address = ctx.get_deposit_address(&eth_address).await;

    assert_ne!(
        address.to_string(),
        "bcrt1quj4mrtx0grz3n2m3axjr65fhe67z8m836f674x".to_string()
    );

    ctx.stop().await
}
