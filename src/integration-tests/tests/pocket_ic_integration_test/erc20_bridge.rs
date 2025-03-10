use std::time::Duration;

use alloy_sol_types::{SolCall, SolConstructor};
use bridge_canister::bridge::Operation;
use bridge_client::{BridgeCanisterClient, Erc20BridgeClient};
use bridge_did::bridge_side::BridgeSide;
use bridge_did::id256::Id256;
use bridge_did::operations::Erc20OpStage;
use bridge_utils::{BTFBridge, UUPSProxy};
use did::{H160, U256};
use erc20_bridge::ops::{Erc20BridgeOpImpl, Erc20OpStageImpl};
use eth_signer::LocalWallet;
use evm_canister_client::EvmCanisterClient;
use ic_stable_structures::Storable as _;

use super::PocketIcTestContext;
use crate::context::stress::{StressTestConfig, erc20};
use crate::context::{CanisterType, TestContext};
use crate::pocket_ic_integration_test::ADMIN;
use crate::utils::CHAIN_ID;

pub struct ContextWithBridges {
    pub context: PocketIcTestContext,
    pub bob_wallet: LocalWallet,
    pub bob_address: H160,
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
        let base_evm = ctx.canisters().external_evm();
        let base_evm_client = EvmCanisterClient::new(ctx.client(base_evm, ctx.admin_name()));

        println!("Deployed external EVM canister: {}", base_evm);
        println!("Deployed EVM canister: {}", ctx.canisters().evm());

        let fee_charge_deployer = ctx.new_wallet(u128::MAX).await.unwrap();
        let deployer_address = fee_charge_deployer.address();
        base_evm_client
            .admin_mint_native_tokens(deployer_address.into(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
        ctx.advance_time(Duration::from_secs(2)).await;
        let expected_fee_charge_address =
            bridge_utils::get_contract_address(fee_charge_deployer.address(), U256::zero());

        let bob_wallet = LocalWallet::random();
        let bob_address: H160 = bob_wallet.address().into();

        // Mint native tokens for bob in both evms
        base_evm_client
            .admin_mint_native_tokens(bob_address.clone(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
        ctx.evm_client(ADMIN)
            .admin_mint_native_tokens(bob_address.clone(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
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
        ctx.evm_client(ADMIN)
            .admin_mint_native_tokens(erc20_bridge_address.clone(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
        base_evm_client
            .admin_mint_native_tokens(erc20_bridge_address.clone(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
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

// Create a second EVM canister (base_evm) instance and create BTFBridge contract on it, // It will play role of external evm
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

    // Check mint operation complete
    let erc20_bridge_client = ctx.context.erc20_bridge_client(ADMIN);

    let amount = 1000_u128;

    // spender should deposit native tokens to btf bridge, to pay fee.
    let wrapped_evm_client = ctx.context.evm_client(ADMIN);
    ctx.context
        .native_token_deposit(
            &wrapped_evm_client,
            ctx.fee_charge_address.clone(),
            &ctx.bob_wallet,
            10_u64.pow(15).into(),
        )
        .await
        .unwrap();

    let base_evm_client = EvmCanisterClient::new(
        ctx.context
            .client(ctx.context.canisters().external_evm(), ADMIN),
    );

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
    ctx.context
        .advance_by_times(Duration::from_secs(2), 30)
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

    let balance = ctx
        .context
        .check_erc20_balance(&ctx.wrapped_token_address, &alice_wallet, None)
        .await
        .unwrap();
    assert_eq!(amount, balance);

    // Wait for mint order removal
    ctx.context
        .advance_by_times(Duration::from_secs(2), 4)
        .await;

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
    let wrapped_evm_client = ctx.context.evm_client(ADMIN);

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
        .eth_get_balance(ctx.fee_charge_address.clone(), did::BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();

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
        .eth_get_balance(ctx.fee_charge_address.clone(), did::BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        init_fee_contract_evm_balance + init_native_balance.clone(),
        fee_contract_evm_balance_after_deposit
    );

    let base_evm_client = EvmCanisterClient::new(
        ctx.context
            .client(ctx.context.canisters().external_evm(), ADMIN),
    );

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

    // Advance time to perform two tasks in erc20-bridge:
    // 1. Minted event collection
    // 2. Mint order removal
    ctx.context
        .advance_by_times(Duration::from_secs(2), 20)
        .await;

    let erc20_bridge_client = ctx.context.erc20_bridge_client(ADMIN);

    let mint_op = erc20_bridge_client
        .get_operations_list(&alice_address, None, None)
        .await
        .unwrap()
        .last()
        .cloned()
        .unwrap();

    if let Erc20OpStage::ConfirmMint { tx_hash, .. } = mint_op.1.stage {
        let receipt = ctx
            .context
            .wait_transaction_receipt(tx_hash.as_ref().unwrap())
            .await
            .unwrap()
            .unwrap();
        eprintln!(
            "TX output: {}",
            String::from_utf8_lossy(&receipt.output.unwrap())
        );
        eprintln!("TX status: {:?}", receipt.status);
    }

    let operation = erc20_bridge_client
        .get_operations_list(&alice_address, None, None)
        .await
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    let operation = Erc20BridgeOpImpl(operation.1);
    assert!(operation.is_complete());

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
    let base_evm_client = EvmCanisterClient::new(
        ctx.context
            .client(ctx.context.canisters().external_evm(), ADMIN),
    );

    // Advance time to perform two tasks in erc20-bridge:
    // 1. Minted event collection
    // 2. Mint order removal
    ctx.context
        .advance_by_times(Duration::from_secs(2), 25)
        .await;

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

    let balance = ctx
        .context
        .check_erc20_balance(&ctx.wrapped_token_address, &alice_wallet, None)
        .await
        .unwrap();
    assert_eq!(0, balance);

    let wrapped_evm_client = ctx.context.evm_client(ADMIN);
    let bridge_canister_evm_balance_after_failed_mint = wrapped_evm_client
        .eth_get_balance(ctx.erc20_bridge_address.clone(), did::BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();

    // Wait for mint order removal
    ctx.context
        .advance_by_times(Duration::from_secs(2), 4)
        .await;

    // Check mint order is not removed
    let erc20_bridge_client = ctx.context.erc20_bridge_client(ADMIN);
    let (_, op) = erc20_bridge_client
        .get_operations_list(&alice_address, None, None)
        .await
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    let stage = Erc20OpStageImpl(op.stage);
    let signed_order = stage.get_signed_mint_order().unwrap();

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

    // check the operation is complete after the successful mint
    let (_, op) = erc20_bridge_client
        .get_operations_list(&alice_address, None, None)
        .await
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    let stage = Erc20BridgeOpImpl(op);
    assert!(stage.is_complete());

    // Check bridge canister balance not changed after user's transaction.
    let bridge_canister_evm_balance_after_user_mint = wrapped_evm_client
        .eth_get_balance(ctx.erc20_bridge_address.clone(), did::BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        bridge_canister_evm_balance_after_failed_mint,
        bridge_canister_evm_balance_after_user_mint
    );
}

#[tokio::test]
async fn native_token_deposit_should_increase_fee_charge_contract_balance() {
    let ctx = ContextWithBridges::new().await;

    let init_erc20_bridge_balance = ctx
        .context
        .evm_client(ADMIN)
        .eth_get_balance(ctx.fee_charge_address.clone(), did::BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();

    // Deposit native tokens to btf bridge.
    let native_token_deposit = 10_000_000_u64;
    let wrapped_evm_client = ctx.context.evm_client(ADMIN);
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
        .evm_client(ADMIN)
        .eth_get_balance(ctx.fee_charge_address.clone(), did::BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        erc20_bridge_balance_after_deposit,
        init_erc20_bridge_balance + native_token_deposit.into()
    );
}

#[tokio::test]
async fn erc20_bridge_stress_test() {
    let context = PocketIcTestContext::new(&[
        CanisterType::Evm,
        CanisterType::ExternalEvm,
        CanisterType::Signature,
        CanisterType::Erc20Bridge,
    ])
    .await;

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
        BridgeSide::Base => ctx.canisters().external_evm(),
        BridgeSide::Wrapped => ctx.canisters().evm(),
    };

    let evm_client = EvmCanisterClient::new(ctx.client(evm, ADMIN));

    let bridge_address = ctx
        .create_contract_on_evm(&evm_client, wallet, btf_input.clone())
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

    ctx.create_contract_on_evm(&evm_client, wallet, proxy_input)
        .await
        .unwrap()
}
