use std::time::Duration;

use alloy_sol_types::{SolCall, SolConstructor};
use bridge_canister::bridge::Operation;
use bridge_client::{BridgeCanisterClient, Erc20BridgeClient};
use bridge_did::bridge_side::BridgeSide;
use bridge_did::id256::Id256;
use bridge_did::operations::{Erc20BridgeOp, Erc20OpStage};
use bridge_utils::{BTFBridge, UUPSProxy};
use did::{H160, U256};
use erc20_bridge::ops::{Erc20BridgeOpImpl, Erc20OpStageImpl};
use eth_signer::LocalWallet;
use ic_stable_structures::Storable as _;

use super::PocketIcTestContext;
use crate::context::stress::{StressTestConfig, erc20};
use crate::context::{CanisterType, TestContext};
use crate::pocket_ic_integration_test::{ADMIN, block_until_succeeds};
use crate::utils::{CHAIN_ID, TestEvm};

pub struct ContextWithBridges {
    pub context: PocketIcTestContext,
    pub bob_wallet: LocalWallet,
    pub bob_address: H160,
    #[allow(dead_code)]
    pub erc20_bridge_address: H160,
    pub base_btf_bridge: H160,
    pub wrapped_btf_bridge: H160,
    pub base_token_address: H160,
    pub wrapped_token_address: H160,
    pub fee_charge_address: H160,
}

impl ContextWithBridges {
    pub async fn new() -> Self {
        let ctx = PocketIcTestContext::new(&CanisterType::EVM_MINTER_TEST_SET).await;

        // Deploy external EVM canister.
        let base_evm_client = ctx.base_evm();
        let wrapped_evm_client = ctx.wrapped_evm();

        let fee_charge_deployer = ctx.new_wallet(u128::MAX).await.unwrap();
        let deployer_address = fee_charge_deployer.address();
        base_evm_client
            .mint_native_tokens(deployer_address.into(), u64::MAX.into())
            .await
            .expect("Failed to mint native tokens");
        ctx.advance_time(Duration::from_secs(2)).await;
        let expected_fee_charge_address =
            bridge_utils::get_contract_address(fee_charge_deployer.address(), U256::zero());

        let bob_wallet = LocalWallet::random();
        let bob_address: H160 = bob_wallet.address().into();

        // Mint native tokens for bob in both evms
        base_evm_client
            .mint_native_tokens(bob_address.clone(), u64::MAX.into())
            .await
            .expect("Failed to mint native tokens");
        wrapped_evm_client
            .mint_native_tokens(bob_address.clone(), u64::MAX.into())
            .await
            .expect("Failed to mint native tokens");
        ctx.advance_time(Duration::from_secs(2)).await;

        // get evm bridge canister address
        let erc20_bridge_client =
            Erc20BridgeClient::new(ctx.client(ctx.canisters().erc20_bridge(), ctx.admin_name()));
        let erc20_bridge_address = erc20_bridge_client
            .get_bridge_canister_evm_address()
            .await
            .unwrap()
            .unwrap();

        // mint native tokens for the erc20-bridge on both EVMs
        println!("Minting native tokens on both EVMs for {erc20_bridge_address}");
        base_evm_client
            .mint_native_tokens(erc20_bridge_address.clone(), u64::MAX.into())
            .await
            .expect("Failed to mint native tokens");
        wrapped_evm_client
            .mint_native_tokens(erc20_bridge_address.clone(), u64::MAX.into())
            .await
            .expect("Failed to mint native tokens");
        ctx.advance_time(Duration::from_secs(2)).await;

        // Deploy wrapped token deployer contracts
        let base_wrapped_token_deployer = H160::default(); // We should not deploy wrapped tokens on base evm.
        let wrapped_wrapped_token_deployer = ctx
            .initialize_wrapped_token_deployer_contract(&bob_wallet)
            .await
            .expect("failed to initialize wrapped token deployer contract");

        // Deploy the BTFBridge contract on the external EVM.
        let base_btf_bridge = create_btf_bridge(
            &ctx,
            &bob_wallet,
            BridgeSide::Base,
            expected_fee_charge_address.into(),
            base_wrapped_token_deployer,
            erc20_bridge_address.clone(),
        )
        .await;
        erc20_bridge_client
            .set_base_btf_bridge_contract(&base_btf_bridge)
            .await
            .unwrap();

        let wrapped_btf_bridge = create_btf_bridge(
            &ctx,
            &bob_wallet,
            BridgeSide::Wrapped,
            expected_fee_charge_address.into(),
            wrapped_wrapped_token_deployer,
            erc20_bridge_address.clone(),
        )
        .await;

        println!("base_btf_bridge: {base_btf_bridge}; wrapped_btf_bridge: {wrapped_btf_bridge}");

        erc20_bridge_client
            .set_btf_bridge_contract(&wrapped_btf_bridge)
            .await
            .unwrap();

        // Deploy FeeCharge contracts.
        let fee_charge_address = ctx
            .initialize_fee_charge_contract_on_evm(
                &base_evm_client,
                &fee_charge_deployer,
                &[base_btf_bridge.clone()],
            )
            .await
            .unwrap();
        assert_eq!(expected_fee_charge_address, fee_charge_address.0);
        let fee_charge_address = ctx
            .initialize_fee_charge_contract(&fee_charge_deployer, &[wrapped_btf_bridge.clone()])
            .await
            .unwrap();
        assert_eq!(expected_fee_charge_address, fee_charge_address.0);

        // Deploy ERC-20 token on external EVM.
        let base_token_address = ctx
            .deploy_test_wtm_token_on_evm(&base_evm_client, &bob_wallet, u64::MAX.into())
            .await
            .unwrap();

        // Deploy Wrapped token on first EVM for the ERC-20 from previous step.
        let token_id = Id256::from_evm_address(&base_token_address, CHAIN_ID as _);
        let wrapped_token_address = ctx
            .create_wrapped_token(
                &ctx.new_wallet(u128::MAX).await.unwrap(),
                &wrapped_btf_bridge,
                token_id,
            )
            .await
            .unwrap();

        Self {
            context: ctx,
            bob_wallet,
            bob_address,
            erc20_bridge_address,
            base_btf_bridge,
            wrapped_btf_bridge,
            base_token_address,
            wrapped_token_address,
            fee_charge_address,
        }
    }

    pub fn bob_address(&self) -> H160 {
        self.bob_address.clone()
    }
}

// Create a second EVM canister (base_evm) instance and create BTFBridge contract on it,
// It will play role of external evm
// Create erc20-bridge instance, initialized with EvmInfos for both EVM canisters.
// Deploy ERC-20 token on external_evm,
// Deploy Wrapped token on first EVM for the ERC-20 from previous step,
// Approve ERC-20 transfer on behalf of some user in external_evm,
// Call BTFBridge::burn() on behalf of the user in external_evm.
// Wait some time for the erc20-bridge see and process it.
// Make sure the tokens minted.
// Make sure SignedMintOrder removed from erc20-bridge after some time.
#[tokio::test]
async fn test_external_bridging() {
    let ctx = ContextWithBridges::new().await;
    // Approve ERC-20 transfer on behalf of some user in base EVM.
    let alice_wallet = ctx.context.new_wallet(u128::MAX).await.unwrap();
    let alice_address: H160 = alice_wallet.address().into();
    let alice_id = Id256::from_evm_address(&alice_address, CHAIN_ID as _);

    // mint native tokens also on base
    let base_evm_client = ctx.context.base_evm();
    base_evm_client
        .mint_native_tokens(ctx.bob_address(), u64::MAX.into())
        .await
        .expect("Failed to mint native tokens");

    // Check mint operation complete
    let erc20_bridge_client = ctx.context.erc20_bridge_client(ADMIN);

    let amount = 1000_u128;

    // spender should deposit native tokens to btf bridge, to pay fee.
    let wrapped_evm_client = ctx.context.wrapped_evm();
    ctx.context
        .native_token_deposit(
            &wrapped_evm_client,
            ctx.fee_charge_address.clone(),
            &ctx.bob_wallet,
            10_u64.pow(15).into(),
        )
        .await
        .unwrap();

    // Advance time to perform two tasks in erc20-bridge:
    // 1. Minted event collection
    // 2. Mint order removal
    ctx.context
        .advance_by_times(Duration::from_secs(2), 20)
        .await;

    let to_token_id = Id256::from_evm_address(&ctx.wrapped_token_address, CHAIN_ID as _);

    let memo = [5 as _; 32];

    let (expected_operation_id, _) = ctx
        .context
        .burn_base_erc_20_tokens(
            &base_evm_client,
            &ctx.bob_wallet,
            &ctx.base_token_address,
            &to_token_id.to_bytes(),
            alice_id,
            &ctx.base_btf_bridge,
            amount,
            Some(memo),
        )
        .await
        .expect("failed to burn base erc20 tokens");

    // Advance time to perform two tasks in erc20-bridge:
    // 1. Minted event collection
    // 2. Mint order removal
    //ctx.context.advance_time(Duration::from_secs(30)).await;
    let ctx_t = ctx.context.clone();
    let alice_wallet_t = alice_wallet.clone();

    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let erc20_bridge_client = ctx.erc20_bridge_client(ADMIN);
            let wallet = alice_wallet_t.clone();
            Box::pin(async move {
                let (operation_id, op) = erc20_bridge_client
                    .get_operation_by_memo_and_user(memo, &wallet.address().into())
                    .await?
                    .ok_or(anyhow::anyhow!("Operation not found"))?;

                if operation_id.as_u64() == expected_operation_id as u64 {
                    if matches!(
                        op,
                        Erc20BridgeOp {
                            stage: Erc20OpStage::TokenMintConfirmed { .. },
                            ..
                        }
                    ) {
                        Ok(())
                    } else {
                        anyhow::bail!("Operation is not in TokenMintConfirmed stage")
                    }
                } else {
                    anyhow::bail!(
                        "Operation id is {operation_id}; expected: {expected_operation_id}"
                    )
                }
            })
        },
        &ctx.context,
        Duration::from_secs(90),
    )
    .await;

    let (operation_id, _) = erc20_bridge_client
        .get_operation_by_memo_and_user(memo, &alice_address)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(operation_id.as_u64(), expected_operation_id as u64);

    let memos = erc20_bridge_client
        .get_memos_by_user_address(&alice_address)
        .await
        .unwrap();
    assert_eq!(memos.len(), 1);
    assert_eq!(memos[0], memo);

    let ctx_t = ctx.context.clone();
    let wrapped_token_address = ctx.wrapped_token_address.clone();
    let alice_wallet = alice_wallet.clone();

    // wait for balance to increase
    block_until_succeeds(
        move || {
            let evm = ctx_t.wrapped_evm.clone();
            let ctx = ctx_t.clone();
            let wrapped_token_address = wrapped_token_address.clone();
            let wallet = alice_wallet.clone();
            Box::pin(async move {
                let balance = ctx
                    .check_erc20_balance_on_evm(&evm, &wrapped_token_address, &wallet, None)
                    .await?;

                if balance == amount {
                    Ok(balance)
                } else {
                    anyhow::bail!("Balance is {balance}; expected: {amount}")
                }
            })
        },
        &ctx.context,
        Duration::from_secs(60),
    )
    .await;

    // Wait for mint order removal
    ctx.context.advance_time(Duration::from_secs(10)).await;

    let operation = erc20_bridge_client
        .get_operations_list(&alice_address, None, None)
        .await
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    let operation = Erc20BridgeOpImpl(operation.1);
    assert!(operation.is_complete());
}

#[tokio::test]
async fn native_token_deposit_increase_and_decrease() {
    let ctx = ContextWithBridges::new().await;

    // Approve ERC-20 transfer on behalf of some user in base EVM.
    let alice_wallet = ctx.context.new_wallet(u128::MAX).await.unwrap();
    let alice_address: H160 = alice_wallet.address().into();
    let alice_id = Id256::from_evm_address(&alice_address, CHAIN_ID as _);
    let amount = 1000_u128;
    let wrapped_evm_client = ctx.context.wrapped_evm();

    let start_native_balance = ctx
        .context
        .native_token_deposit_balance(
            &wrapped_evm_client,
            ctx.fee_charge_address.clone(),
            ctx.bob_address(),
        )
        .await;
    assert_eq!(start_native_balance, U256::zero());

    let init_fee_contract_evm_balance = wrapped_evm_client
        .eth_get_balance(&ctx.fee_charge_address, did::BlockNumber::Latest)
        .await
        .expect("Failed to get balance");

    // spender should deposit native tokens to btf bridge, to pay fee.
    let native_balance_after_deposit = 10_u64.pow(15);
    let init_native_balance = ctx
        .context
        .native_token_deposit(
            &wrapped_evm_client,
            ctx.fee_charge_address.clone(),
            &ctx.bob_wallet,
            native_balance_after_deposit.into(),
        )
        .await
        .unwrap();
    assert_eq!(
        init_native_balance.0.to::<u64>(),
        native_balance_after_deposit
    );

    let queried_balance = ctx
        .context
        .native_token_deposit_balance(
            &wrapped_evm_client,
            ctx.fee_charge_address.clone(),
            ctx.bob_address(),
        )
        .await;
    assert_eq!(queried_balance.0.to::<u64>(), native_balance_after_deposit);

    let fee_contract_evm_balance_after_deposit = wrapped_evm_client
        .eth_get_balance(&ctx.fee_charge_address, did::BlockNumber::Latest)
        .await
        .expect("Failed to get balance");

    assert_eq!(
        init_fee_contract_evm_balance + init_native_balance.clone(),
        fee_contract_evm_balance_after_deposit
    );

    let base_evm_client = ctx.context.base_evm();

    // Advance time to perform two tasks in erc20-bridge:
    // 1. Minted event collection
    // 2. Mint order removal
    ctx.context
        .advance_by_times(Duration::from_secs(2), 10)
        .await;

    let to_token_id = Id256::from_evm_address(&ctx.wrapped_token_address, CHAIN_ID as _);
    // Perform an operation to pay a fee for it.
    ctx.context
        .burn_base_erc_20_tokens(
            &base_evm_client,
            &ctx.bob_wallet,
            &ctx.base_token_address,
            &to_token_id.to_bytes(),
            alice_id,
            &ctx.base_btf_bridge,
            amount,
            None,
        )
        .await
        .unwrap();

    let alice_address_t = alice_wallet.clone().address();
    let ctx_t = ctx.context.clone();

    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let erc20_bridge_client = ctx.erc20_bridge_client(ADMIN);
            let alice_address = alice_address_t;
            Box::pin(async move {
                let (_, op) = erc20_bridge_client
                    .get_operations_list(&alice_address.into(), None, None)
                    .await?
                    .last()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Operation not found"))?;

                let stage = Erc20BridgeOpImpl(op);
                if stage.is_complete() {
                    Ok(())
                } else {
                    anyhow::bail!("Operation is not complete; {stage:?}")
                }
            })
        },
        &ctx.context,
        Duration::from_secs(120),
    )
    .await;

    // Check fee charged
    let native_balance_after_mint = ctx
        .context
        .native_token_deposit_balance(
            &wrapped_evm_client,
            ctx.fee_charge_address.clone(),
            ctx.bob_address(),
        )
        .await;
    assert!(native_balance_after_mint > U256::zero());
    assert!(native_balance_after_mint < init_native_balance);
}

#[tokio::test]
async fn mint_should_fail_if_not_enough_tokens_on_fee_deposit() {
    let ctx = ContextWithBridges::new().await;
    // Approve ERC-20 transfer on behalf of some user in base EVM.
    let alice_wallet = ctx.context.new_wallet(u128::MAX).await.unwrap();
    let alice_address: H160 = alice_wallet.address().into();
    let alice_id = Id256::from_evm_address(&alice_address, CHAIN_ID as _);
    let amount = 1000_u128;

    // spender should deposit native tokens to btf bridge, to pay fee.
    let base_evm_client = ctx.context.base_evm();

    let to_token_id = Id256::from_evm_address(&ctx.wrapped_token_address, CHAIN_ID as _);

    ctx.context
        .burn_base_erc_20_tokens(
            &base_evm_client,
            &ctx.bob_wallet,
            &ctx.base_token_address,
            &to_token_id.to_bytes(),
            alice_id,
            &ctx.base_btf_bridge,
            amount,
            None,
        )
        .await
        .unwrap();

    // Advance time to perform two tasks in erc20-bridge:
    // 1. Minted event collection
    // 2. Mint order removal
    ctx.context
        .advance_by_times(Duration::from_secs(2), 25)
        .await;

    let ctx_t = ctx.context.clone();
    let wrapped_token_address = ctx.wrapped_token_address.clone();
    let alice_wallet_t = alice_wallet.clone();

    let expected_amount = 0;

    // wait for balance to increase
    block_until_succeeds(
        move || {
            let evm = ctx_t.wrapped_evm.clone();
            let ctx = ctx_t.clone();
            let wrapped_token_address = wrapped_token_address.clone();
            let wallet = alice_wallet_t.clone();
            Box::pin(async move {
                let balance = ctx
                    .check_erc20_balance_on_evm(&evm, &wrapped_token_address, &wallet, None)
                    .await?;

                if balance == expected_amount {
                    Ok(balance)
                } else {
                    anyhow::bail!("Balance is {balance}; expected: {expected_amount}")
                }
            })
        },
        &ctx.context,
        Duration::from_secs(60),
    )
    .await;

    let alice_address_t = alice_wallet.clone().address();
    let ctx_t = ctx.context.clone();

    let signed_order = block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let erc20_bridge_client = ctx.erc20_bridge_client(ADMIN);
            let alice_address = alice_address_t;
            Box::pin(async move {
                let (_, op) = erc20_bridge_client
                    .get_operations_list(&alice_address.into(), None, None)
                    .await
                    .unwrap()
                    .last()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Operation not found"))?;

                let stage = Erc20OpStageImpl(op.stage);
                let signed_order = stage
                    .get_signed_mint_order()
                    .ok_or_else(|| anyhow::anyhow!("Signed order not found"))?;

                Ok(signed_order.clone())
            })
        },
        &ctx.context,
        Duration::from_secs(60),
    )
    .await;

    ctx.context
        .batch_mint_erc_20_with_order(
            &ctx.bob_wallet,
            &ctx.wrapped_btf_bridge,
            signed_order.clone(),
        )
        .await
        .unwrap();

    ctx.context
        .advance_by_times(Duration::from_secs(2), 10)
        .await;

    let alice_address_t = alice_wallet.clone().address();
    let ctx_t = ctx.context.clone();

    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let erc20_bridge_client = ctx.erc20_bridge_client(ADMIN);
            let alice_address = alice_address_t;
            Box::pin(async move {
                let (_, op) = erc20_bridge_client
                    .get_operations_list(&alice_address.into(), None, None)
                    .await
                    .unwrap()
                    .last()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Operation not found"))?;

                let stage = Erc20BridgeOpImpl(op);
                if stage.is_complete() {
                    Ok(())
                } else {
                    anyhow::bail!("Operation is not complete; {stage:?}")
                }
            })
        },
        &ctx.context,
        Duration::from_secs(120),
    )
    .await;
}

#[tokio::test]
async fn native_token_deposit_should_increase_fee_charge_contract_balance() {
    let ctx = ContextWithBridges::new().await;

    let init_erc20_bridge_balance = ctx
        .context
        .base_evm()
        .eth_get_balance(&ctx.fee_charge_address, did::BlockNumber::Latest)
        .await
        .expect("Failed to get balance");

    // Deposit native tokens to btf bridge.
    let native_token_deposit = 10_000_000_u64;
    let wrapped_evm_client = ctx.context.wrapped_evm();
    ctx.context
        .native_token_deposit(
            &wrapped_evm_client,
            ctx.fee_charge_address.clone(),
            &ctx.bob_wallet,
            native_token_deposit.into(),
        )
        .await
        .unwrap();

    let erc20_bridge_balance_after_deposit = ctx
        .context
        .wrapped_evm()
        .eth_get_balance(&ctx.fee_charge_address, did::BlockNumber::Latest)
        .await
        .expect("Failed to get balance");

    assert_eq!(
        erc20_bridge_balance_after_deposit,
        init_erc20_bridge_balance + native_token_deposit.into()
    );
}

#[tokio::test]
async fn erc20_bridge_stress_test() {
    let context = PocketIcTestContext::new(&[CanisterType::Erc20Bridge]).await;

    let config = StressTestConfig {
        users_number: 5,
        user_deposits_per_token: 3,
        init_user_balance: 2u64.pow(30).into(),
        operation_amount: 2u64.pow(20).into(),
        operation_timeout: Duration::from_secs(30),
        wait_per_iteration: Duration::from_secs(1),
        charge_fee: true,
    };

    // If set more then one token, tests probably will fail because of
    // parallel tx nonces calculation issue.
    erc20::stress_test_erc20_bridge_with_ctx(context, 1, config).await;
}

#[tokio::test]
async fn test_should_get_batch_mint_errors_if_mint_fails() {
    let ctx = ContextWithBridges::new().await;
    // Approve ERC-20 transfer on behalf of some user in base EVM.
    let alice_wallet = ctx.context.new_wallet(u128::MAX).await.unwrap();
    let alice_address: H160 = alice_wallet.address().into();
    let alice_id = Id256::from_evm_address(&alice_address, CHAIN_ID as _);

    let amount = 1000_u128;

    // spender should deposit native tokens to btf bridge, to pay fee.
    let wrapped_evm_client = ctx.context.wrapped_evm();
    ctx.context
        .native_token_deposit(
            &wrapped_evm_client,
            ctx.fee_charge_address.clone(),
            &ctx.bob_wallet,
            10_u64.pow(15).into(),
        )
        .await
        .unwrap();

    let base_evm_client = ctx.context.base_evm();

    // Advance time to perform two tasks in erc20-bridge:
    // 1. Minted event collection
    // 2. Mint order removal
    ctx.context
        .advance_by_times(Duration::from_secs(2), 20)
        .await;

    let to_token_id = Id256::from_evm_address(
        &H160::from_hex_str("0x6d3bd4e50c9aae16edccb5d1aef32f9271aed9b0").unwrap(),
        1u64 as _,
    ); // NOTE: this chain id will cause a mint error

    let memo = [5 as _; 32];

    let (expected_operation_id, _) = ctx
        .context
        .burn_base_erc_20_tokens(
            &base_evm_client,
            &ctx.bob_wallet,
            &ctx.base_token_address,
            &to_token_id.to_bytes(),
            alice_id,
            &ctx.base_btf_bridge,
            amount,
            Some(memo),
        )
        .await
        .expect("failed to burn base erc20 tokens");

    let ctx_t = ctx.context.clone();
    let alice_wallet_t = alice_wallet.clone();

    let (tx_hash, mint_results) = block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let erc20_bridge_client = ctx.erc20_bridge_client(ADMIN);
            let wallet = alice_wallet_t.clone();
            Box::pin(async move {
                let (operation_id, op) = erc20_bridge_client
                    .get_operation_by_memo_and_user(memo, &wallet.address().into())
                    .await?
                    .ok_or(anyhow::anyhow!("Operation not found"))?;

                if operation_id.as_u64() == expected_operation_id as u64 {
                    match op {
                        Erc20BridgeOp {
                            stage:
                                Erc20OpStage::WaitForMintConfirm {
                                    tx_hash,
                                    mint_results,
                                    ..
                                },
                            ..
                        } => Ok((tx_hash, mint_results)),
                        _ => anyhow::bail!("Expected WaitForMintConfirm stage: {op:?}"),
                    }
                } else {
                    anyhow::bail!(
                        "Operation id is {operation_id}; expected: {expected_operation_id}"
                    )
                }
            })
        },
        &ctx.context,
        Duration::from_secs(90),
    )
    .await;

    println!("Result: {tx_hash:?} | {mint_results:?}");
    assert!(tx_hash.is_none(), "mint tx should not have been sent");
    assert_eq!(mint_results.len(), 1);
    assert_eq!(
        mint_results[0],
        bridge_did::batch_mint_result::BatchMintErrorCode::Reverted(
            "Invalid token pair".to_string()
        )
    );
}

async fn create_btf_bridge(
    ctx: &PocketIcTestContext,
    wallet: &LocalWallet,
    side: BridgeSide,
    fee_charge: H160,
    wrapped_token_deployer: H160,
    minter_address: H160,
) -> H160 {
    let is_wrapped = match side {
        BridgeSide::Base => false,
        BridgeSide::Wrapped => true,
    };

    let mut btf_input = BTFBridge::BYTECODE.to_vec();
    let constructor = BTFBridge::constructorCall {}.abi_encode();
    btf_input.extend_from_slice(&constructor);

    let evm = match side {
        BridgeSide::Base => ctx.base_evm(),
        BridgeSide::Wrapped => ctx.wrapped_evm(),
    };

    let bridge_address = ctx
        .create_contract_on_evm(&evm, wallet, btf_input.clone())
        .await
        .unwrap();

    let init_data = BTFBridge::initializeCall {
        minterAddress: minter_address.into(),
        feeChargeAddress: fee_charge.into(),
        wrappedTokenDeployer: wrapped_token_deployer.into(),
        isWrappedSide: is_wrapped,
        owner: [0; 20].into(),
        controllers: vec![],
    }
    .abi_encode();

    let mut proxy_input = UUPSProxy::BYTECODE.to_vec();
    let constructor = UUPSProxy::constructorCall {
        _implementation: bridge_address.into(),
        _data: init_data.into(),
    }
    .abi_encode();
    proxy_input.extend_from_slice(&constructor);

    ctx.create_contract_on_evm(&evm, wallet, proxy_input)
        .await
        .unwrap()
}
