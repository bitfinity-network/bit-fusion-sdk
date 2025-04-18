use std::sync::Arc;
use std::time::Duration;

use bitcoin::Amount;
use bridge_did::operations::BtcBridgeOp;
use btc_bridge::canister::eth_address_to_subaccount;
use icrc_client::account::Account;

use crate::context::TestContext;
use crate::context::btc_bridge::BtcContext;
use crate::context::stress::StressTestConfig;
use crate::pocket_ic_integration_test::{ALICE, block_until_succeeds};
use crate::utils::btc_wallet::BtcWallet;

const CKBTC_LEDGER_FEE: u64 = 1_000;
const KYT_FEE: u64 = 2_000;

#[tokio::test]
async fn btc_to_erc20_test() {
    let ctx = BtcContext::pocket_ic().await;
    let ctx = Arc::new(ctx);

    let deposit_value = 100_000_000;

    let wallet = Arc::new(
        ctx.context
            .new_wallet(u128::MAX)
            .await
            .expect("Failed to create a wallet"),
    );
    let my_eth_address = wallet.address().into();

    ctx.mint_admin_wrapped_btc(deposit_value, &wallet, &my_eth_address, 0)
        .await
        .expect("Mint failed");

    // wait for minted balance to be updated
    let ctx_t = ctx.clone();
    let wallet_t = wallet.clone();
    let minted = block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let wallet = wallet_t.clone();
            Box::pin(async move {
                let balance = ctx.erc20_balance_of(&wallet, None).await?;

                if balance > 0 {
                    Ok(balance)
                } else {
                    anyhow::bail!("Balance is 0")
                }
            })
        },
        &ctx.context,
        Duration::from_secs(60),
    )
    .await;

    let expected_balance = (deposit_value - KYT_FEE - CKBTC_LEDGER_FEE) as u128;
    assert_eq!(minted, expected_balance);

    let canister_balance = ctx
        .icrc_balance_of(Account {
            owner: ctx.context.canisters.btc_bridge(),
            subaccount: None,
        })
        .await
        .expect("get canister balance failed");
    assert_eq!(canister_balance, expected_balance);

    ctx.stop().await;
}

#[tokio::test]
async fn test_get_btc_address_from_bridge() {
    let ctx = BtcContext::pocket_ic().await;

    let wallet = Arc::new(
        ctx.context
            .new_wallet(u128::MAX)
            .await
            .expect("Failed to create a wallet"),
    );

    let caller_eth_address = wallet.address().into();

    let deposit_account = Account {
        owner: ctx.context.canisters.btc_bridge(),
        subaccount: Some(eth_address_to_subaccount(&caller_eth_address).0),
    };
    let deposit_address = ctx
        .get_btc_address(deposit_account)
        .await
        .expect("get btc address failed");

    let deposit_address_anonymous = ctx
        .get_btc_address_from_bridge(deposit_account)
        .await
        .expect("get btc address failed");

    assert_eq!(deposit_address, deposit_address_anonymous);

    ctx.stop().await;
}

#[tokio::test]
async fn test_should_mint_erc20_with_several_concurrent_btc_transactions() {
    let ctx = Arc::new(BtcContext::pocket_ic().await);

    let deposit_value = 100_000_000;

    let wallet = Arc::new(
        ctx.context
            .new_wallet(u128::MAX)
            .await
            .expect("Failed to create a wallet"),
    );

    let tx_count: u64 = 40;
    assert!(deposit_value % tx_count == 0);

    let tx_value = deposit_value / tx_count;

    let caller_eth_address = wallet.address().into();

    for tx_count in 0..tx_count {
        let deposit_account = Account {
            owner: ctx.context.canisters.btc_bridge(),
            subaccount: Some(eth_address_to_subaccount(&caller_eth_address).0),
        };
        let deposit_address = ctx
            .get_btc_address(deposit_account)
            .await
            .expect("get btc address failed");
        let utxo = ctx
            .get_funding_utxo(&deposit_address, Amount::from_sat(tx_value))
            .await
            .expect("Failed to get funding utxo");

        println!("Pushed tx {tx_count}: {utxo:?}");

        ctx.wait_for_blocks(1).await;
    }

    ctx.wait_for_blocks(6).await;

    // deposit
    let wallet_t = wallet.clone();
    let ctx_t = ctx.clone();
    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let wallet = wallet_t.clone();
            Box::pin(async move {
                let caller_eth_address = wallet.address().into();
                ctx.btc_to_erc20(&wallet, &caller_eth_address, [0; 32], 0)
                    .await
            })
        },
        &ctx.context,
        Duration::from_secs(120),
    )
    .await;

    let ctx_t = ctx.clone();
    let wallet_t = wallet.clone();

    let expected_balance = (deposit_value - (KYT_FEE * tx_count) - CKBTC_LEDGER_FEE) as u128;
    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let wallet = wallet_t.clone();
            Box::pin(async move {
                let balance = ctx.erc20_balance_of(&wallet, None).await?;

                if balance == expected_balance {
                    Ok(balance)
                } else {
                    anyhow::bail!("Balance is {balance}; expected: {expected_balance}")
                }
            })
        },
        &ctx.context,
        Duration::from_secs(120),
    )
    .await;

    let canister_balance = ctx
        .icrc_balance_of(Account {
            owner: ctx.context.canisters.btc_bridge(),
            subaccount: None,
        })
        .await
        .expect("get canister balance failed");
    assert_eq!(canister_balance, expected_balance);

    ctx.stop().await;
}

#[tokio::test]
async fn test_should_mint_erc20_with_several_tx_from_different_wallets() {
    let ctx = Arc::new(BtcContext::pocket_ic().await);

    let deposit_value = 100_000_000;
    let wallets_count = 12;

    let mut wallets = Vec::new();
    for _ in 0..wallets_count {
        let wallet = Arc::new(
            ctx.context
                .new_wallet(u128::MAX)
                .await
                .expect("Failed to create a wallet"),
        );
        wallets.push(wallet);
    }

    for wallet in &wallets {
        let caller_eth_address = wallet.address().into();
        let deposit_account = Account {
            owner: ctx.context.canisters.btc_bridge(),
            subaccount: Some(eth_address_to_subaccount(&caller_eth_address).0),
        };
        let deposit_address = ctx
            .get_btc_address(deposit_account)
            .await
            .expect("get btc address failed");
        let utxo = ctx
            .get_funding_utxo(&deposit_address, Amount::from_sat(deposit_value))
            .await
            .expect("Failed to get funding utxo");

        println!("Pushed tx from {deposit_address}: {utxo:?}");

        ctx.wait_for_blocks(1).await;
    }

    for wallet in &wallets {
        println!("making deposit for {}", wallet.address());
        // deposit
        let wallet_t = wallet.clone();
        let ctx_t = ctx.clone();
        block_until_succeeds(
            move || {
                let ctx = ctx_t.clone();
                let wallet = wallet_t.clone();
                Box::pin(async move {
                    let caller_eth_address = wallet.address().into();
                    ctx.btc_to_erc20(&wallet, &caller_eth_address, [0; 32], 0)
                        .await
                })
            },
            &ctx.context,
            Duration::from_secs(120),
        )
        .await;

        let ctx_t = ctx.clone();
        let wallet_t = wallet.clone();

        let expected_balance = (deposit_value - KYT_FEE - CKBTC_LEDGER_FEE) as u128;

        block_until_succeeds(
            move || {
                let ctx = ctx_t.clone();
                let wallet = wallet_t.clone();
                Box::pin(async move {
                    let balance = ctx.erc20_balance_of(&wallet, None).await?;

                    if balance == expected_balance {
                        Ok(balance)
                    } else {
                        anyhow::bail!("Balance is {balance}; expected: {expected_balance}")
                    }
                })
            },
            &ctx.context,
            Duration::from_secs(120),
        )
        .await;
    }

    ctx.stop().await;
}

#[tokio::test]
async fn test_should_deposit_and_withdraw_btc() {
    let ctx = BtcContext::pocket_ic().await;
    let ctx = Arc::new(ctx);

    let deposit_value = 100_000_000;
    let deposit_amount = Amount::from_sat(deposit_value);

    let wallet = Arc::new(
        ctx.context
            .new_wallet(u128::MAX)
            .await
            .expect("Failed to create a wallet"),
    );

    let btc_wallet = Arc::new(BtcWallet::new_random());
    println!("BTC Wallet: {}", btc_wallet.address);

    // ! deposit
    let wallet_t = wallet.clone();
    let ctx_t = ctx.clone();
    let btc_wallet_t = btc_wallet.clone();
    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let wallet = wallet_t.clone();
            let btc_wallet = btc_wallet_t.clone();
            Box::pin(async move {
                let caller_eth_address = wallet.address().into();
                ctx.deposit_btc(
                    &wallet,
                    &btc_wallet,
                    deposit_amount,
                    &caller_eth_address,
                    [0; 32],
                    0,
                )
                .await
            })
        },
        &ctx.context,
        Duration::from_secs(120),
    )
    .await;

    // wait for minted balance to be updated
    let ctx_t = ctx.clone();
    let wallet_t = wallet.clone();
    let expected_balance = (deposit_value - KYT_FEE - CKBTC_LEDGER_FEE) as u128;

    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let wallet = wallet_t.clone();
            Box::pin(async move {
                let balance = ctx.erc20_balance_of(&wallet, None).await?;

                if balance == expected_balance {
                    Ok(balance)
                } else {
                    anyhow::bail!("Balance is {balance}; expected: {expected_balance}")
                }
            })
        },
        &ctx.context,
        Duration::from_secs(60),
    )
    .await;

    // ! withdraw
    let recipient = btc_wallet.address.clone();
    let withdraw_amount = Amount::from_sat(50_000_000);
    let prev_btc_balance = ctx.btc_balance(&recipient).await;

    ctx.withdraw_btc(&wallet, &recipient, withdraw_amount, None)
        .await
        .expect("withdraw failed");

    ctx.context.advance_time(Duration::from_secs(1)).await;
    ctx.wait_for_blocks(6).await;

    // expected ERC20 balance
    let ctx_t = ctx.clone();
    let wallet_t = wallet.clone();
    let expected_balance =
        (deposit_value - KYT_FEE - CKBTC_LEDGER_FEE - withdraw_amount.to_sat()) as u128;

    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let wallet = wallet_t.clone();
            Box::pin(async move {
                let balance = ctx.erc20_balance_of(&wallet, None).await?;

                if balance == expected_balance {
                    Ok(balance)
                } else {
                    anyhow::bail!("Balance is {balance}; expected: {expected_balance}")
                }
            })
        },
        &ctx.context,
        Duration::from_secs(60),
    )
    .await;

    // check btc balanace
    let ctx_t = ctx.clone();
    let expected_balance = prev_btc_balance + withdraw_amount - Amount::from_sat(4_017); // fee

    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let recipient = recipient.clone();
            Box::pin(async move {
                let btc_balance = ctx.btc_balance(&recipient).await;

                if btc_balance == expected_balance {
                    Ok(btc_balance)
                } else {
                    anyhow::bail!("Balance is {btc_balance}; expected: {expected_balance}")
                }
            })
        },
        &ctx.context,
        Duration::from_secs(60),
    )
    .await;

    ctx.stop().await;
}

#[tokio::test]
#[serial_test::serial]
async fn test_btc_bridge_stress_test() {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    let context = crate::pocket_ic_integration_test::PocketIcTestContext::new_with(
        &crate::context::CanisterType::BTC_CANISTER_SET,
        |builder| {
            builder
                .with_ii_subnet()
                .with_bitcoin_subnet()
                .with_bitcoind_addr(SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                    18444,
                ))
        },
        true,
    )
    .await;

    let config = StressTestConfig {
        users_number: 5,
        user_deposits_per_token: 1,
        init_user_balance: 1_000_000u64.into(), // 0.01 BTC
        operation_amount: 500_000u64.into(),    // 0.005 BTC
        operation_timeout: Duration::from_secs(120),
        wait_per_iteration: Duration::from_secs(10),
        charge_fee: false,
    };

    crate::context::stress::btc::stress_test_btc_bridge_with_ctx(context, config).await;
}

#[tokio::test]
async fn test_should_track_deposit_and_withdrawal_operation() {
    let ctx = BtcContext::pocket_ic().await;
    let ctx = Arc::new(ctx);

    let deposit_value = 100_000_000;
    let deposit_amount = Amount::from_sat(deposit_value);

    let wallet = Arc::new(
        ctx.context
            .new_wallet(u128::MAX)
            .await
            .expect("Failed to create a wallet"),
    );

    let btc_wallet = Arc::new(BtcWallet::new_random());
    println!("BTC Wallet: {}", btc_wallet.address);

    // ! deposit
    let wallet_t = wallet.clone();
    let ctx_t = ctx.clone();
    let btc_wallet_t = btc_wallet.clone();

    let deposit_memo = [77; 32];
    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let wallet = wallet_t.clone();
            let btc_wallet = btc_wallet_t.clone();
            Box::pin(async move {
                let caller_eth_address = wallet.address().into();
                ctx.deposit_btc(
                    &wallet,
                    &btc_wallet,
                    deposit_amount,
                    &caller_eth_address,
                    deposit_memo,
                    0,
                )
                .await
            })
        },
        &ctx.context,
        Duration::from_secs(120),
    )
    .await;

    // wait for minted balance to be updated
    let ctx_t = ctx.clone();
    let wallet_t = wallet.clone();
    let expected_balance = (deposit_value - KYT_FEE - CKBTC_LEDGER_FEE) as u128;

    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let wallet = wallet_t.clone();
            Box::pin(async move {
                let balance = ctx.erc20_balance_of(&wallet, None).await?;

                if balance == expected_balance {
                    Ok(balance)
                } else {
                    anyhow::bail!("Balance is {balance}; expected: {expected_balance}")
                }
            })
        },
        &ctx.context,
        Duration::from_secs(60),
    )
    .await;

    let (deposit_op_id, deposit_operation) = ctx
        .context
        .btc_bridge_client(ALICE)
        .get_operation_by_memo_and_user(deposit_memo, &wallet.address().into())
        .await
        .expect("error trying to retrieve deposition operation")
        .expect("deposit operation not found");

    assert!(
        matches!(
            &deposit_operation,
            BtcBridgeOp::WaitForErc20MintConfirm { .. } | BtcBridgeOp::Erc20MintConfirmed { .. }
        ),
        "Incorrect operation result: {deposit_operation:?}"
    );

    // ! withdraw
    let recipient = btc_wallet.address.clone();
    let withdraw_amount = Amount::from_sat(50_000_000);
    let prev_btc_balance = ctx.btc_balance(&recipient).await;
    let withdrawal_memo = [101; 32];

    ctx.withdraw_btc(&wallet, &recipient, withdraw_amount, Some(withdrawal_memo))
        .await
        .expect("withdraw failed");

    ctx.context.advance_time(Duration::from_secs(1)).await;
    ctx.wait_for_blocks(6).await;

    // expected ERC20 balance
    let ctx_t = ctx.clone();
    let wallet_t = wallet.clone();
    let expected_balance =
        (deposit_value - KYT_FEE - CKBTC_LEDGER_FEE - withdraw_amount.to_sat()) as u128;

    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let wallet = wallet_t.clone();
            Box::pin(async move {
                let balance = ctx.erc20_balance_of(&wallet, None).await?;

                if balance == expected_balance {
                    Ok(balance)
                } else {
                    anyhow::bail!("Balance is {balance}; expected: {expected_balance}")
                }
            })
        },
        &ctx.context,
        Duration::from_secs(60),
    )
    .await;

    // check btc balanace
    let ctx_t = ctx.clone();
    let expected_balance = prev_btc_balance + withdraw_amount - Amount::from_sat(4_017); // fee

    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let recipient = recipient.clone();
            Box::pin(async move {
                let btc_balance = ctx.btc_balance(&recipient).await;

                if btc_balance == expected_balance {
                    Ok(btc_balance)
                } else {
                    anyhow::bail!("Balance is {btc_balance}; expected: {expected_balance}")
                }
            })
        },
        &ctx.context,
        Duration::from_secs(60),
    )
    .await;

    let (withdrawal_op_id, withdrawal_operation) = ctx
        .context
        .btc_bridge_client(ALICE)
        .get_operation_by_memo_and_user(withdrawal_memo, &wallet.address().into())
        .await
        .expect("error trying to retrieve withdrawal operation")
        .expect("withdrawal operation not found");

    assert!(
        matches!(&withdrawal_operation, BtcBridgeOp::BtcWithdrawConfirmed { eth_address } if *eth_address == wallet.address().into()),
        "Incorrect operation result: {withdrawal_operation:?}"
    );

    let operation_list = ctx
        .context
        .btc_bridge_client(ALICE)
        .get_operations_list(&wallet.address().into(), None, None)
        .await
        .expect("failed to get operation list");

    assert!(operation_list.iter().any(|(id, _)| *id == deposit_op_id));
    assert!(operation_list.iter().any(|(id, _)| *id == withdrawal_op_id));

    ctx.stop().await;
}
