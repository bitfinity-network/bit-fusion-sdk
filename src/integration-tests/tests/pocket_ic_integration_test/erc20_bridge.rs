use std::time::Duration;

use alloy_sol_types::{SolCall, SolConstructor};
use bridge_canister::bridge::Operation;
use bridge_client::{BridgeCanisterClient, Erc20BridgeClient};
use bridge_did::id256::Id256;
use bridge_utils::evm_bridge::BridgeSide;
use bridge_utils::{BFTBridge, UUPSProxy};
use did::{H160, U256, U64};
use erc20_bridge::ops::Erc20OpStage;
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use evm_canister_client::EvmCanisterClient;
use ic_stable_structures::Storable as _;

use super::PocketIcTestContext;
use crate::context::{CanisterType, TestContext};
use crate::pocket_ic_integration_test::{TestWTM, ADMIN};
use crate::utils::CHAIN_ID;

pub struct ContextWithBridges {
    pub context: PocketIcTestContext,
    pub bob_wallet: Wallet<'static, SigningKey>,
    pub bob_address: H160,
    pub erc20_bridge_address: H160,
    pub base_bft_bridge: H160,
    pub wrapped_bft_bridge: H160,
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
            .mint_native_tokens(deployer_address.into(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
        ctx.advance_time(Duration::from_secs(2)).await;
        let expected_fee_charge_address =
            ethers_core::utils::get_contract_address(fee_charge_deployer.address(), 0);

        let mut rng = rand::thread_rng();

        let bob_wallet = Wallet::new(&mut rng);
        let bob_address: H160 = bob_wallet.address().into();

        // Mint native tokens for bob in both evms
        base_evm_client
            .mint_native_tokens(bob_address.clone(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
        ctx.evm_client(ADMIN)
            .mint_native_tokens(bob_address.clone(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
        ctx.advance_time(Duration::from_secs(2)).await;

        // get evm bridge canister address
        let erc20_bridge_client =
            Erc20BridgeClient::new(ctx.client(ctx.canisters().erc20_bridge(), ADMIN));
        let erc20_bridge_address = erc20_bridge_client
            .get_bridge_canister_evm_address()
            .await
            .unwrap()
            .unwrap();

        // mint native tokens for the erc20-bridge on both EVMs
        println!("Minting native tokens on both EVMs for {erc20_bridge_address}");
        ctx.evm_client(ADMIN)
            .mint_native_tokens(erc20_bridge_address.clone(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
        base_evm_client
            .mint_native_tokens(erc20_bridge_address.clone(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
        ctx.advance_time(Duration::from_secs(2)).await;

        // Deploy the BFTBridge contract on the external EVM.
        let base_bft_bridge = create_bft_bridge(
            &ctx,
            &bob_wallet,
            BridgeSide::Base,
            expected_fee_charge_address.into(),
            erc20_bridge_address.clone(),
        )
        .await;
        erc20_bridge_client
            .set_base_bft_bridge_contract(&base_bft_bridge)
            .await
            .unwrap();

        let wrapped_bft_bridge = create_bft_bridge(
            &ctx,
            &bob_wallet,
            BridgeSide::Wrapped,
            expected_fee_charge_address.into(),
            erc20_bridge_address.clone(),
        )
        .await;
        erc20_bridge_client
            .set_bft_bridge_contract(&wrapped_bft_bridge)
            .await
            .unwrap();

        // Deploy FeeCharge contracts.
        let fee_charge_address = ctx
            .initialize_fee_charge_contract_on_evm(
                &base_evm_client,
                &fee_charge_deployer,
                &[base_bft_bridge.clone()],
            )
            .await
            .unwrap();
        assert_eq!(expected_fee_charge_address, fee_charge_address.0);
        let fee_charge_address = ctx
            .initialize_fee_charge_contract(&fee_charge_deployer, &[wrapped_bft_bridge.clone()])
            .await
            .unwrap();
        assert_eq!(expected_fee_charge_address, fee_charge_address.0);

        // Deploy ERC-20 token on external EVM.
        let mut erc20_input = TestWTM::BYTECODE.to_vec();
        let constructor = TestWTM::constructorCall {
            initialSupply: U256::from(u64::MAX).into(),
        }
        .abi_encode();
        erc20_input.extend_from_slice(&constructor);

        let nonce = base_evm_client
            .account_basic(bob_address.clone())
            .await
            .unwrap()
            .nonce;
        let tx = ctx.signed_transaction(&bob_wallet, None, nonce, 0, erc20_input);
        let base_token_address = {
            let hash = base_evm_client
                .send_raw_transaction(tx)
                .await
                .unwrap()
                .unwrap();

            let receipt = ctx
                .wait_transaction_receipt_on_evm(&base_evm_client, &hash)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(receipt.status, Some(U64::one()));

            receipt.contract_address.unwrap()
        };

        // Deploy Wrapped token on first EVM for the ERC-20 from previous step.
        let token_id = Id256::from_evm_address(&base_token_address, CHAIN_ID as _);
        let wrapped_token_address = ctx
            .create_wrapped_token(
                &ctx.new_wallet(u128::MAX).await.unwrap(),
                &wrapped_bft_bridge,
                token_id,
            )
            .await
            .unwrap();

        Self {
            context: ctx,
            bob_wallet,
            bob_address,
            erc20_bridge_address,
            base_bft_bridge,
            wrapped_bft_bridge,
            base_token_address,
            wrapped_token_address,
            fee_charge_address,
        }
    }

    pub fn bob_address(&self) -> H160 {
        self.bob_address.clone()
    }
}

// Create a second EVM canister (base_evm) instance and create BFTBridge contract on it, // It will play role of external evm
// Create erc20-bridge instance, initialized with EvmInfos for both EVM canisters.
// Deploy ERC-20 token on external_evm,
// Deploy Wrapped token on first EVM for the ERC-20 from previous step,
// Approve ERC-20 transfer on behalf of some user in external_evm,
// Call BFTBridge::burn() on behalf of the user in external_evm.
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
    let erc20_bridge_client = ctx.context.erc_bridge_client(ADMIN);

    let amount = 1000_u128;

    // spender should deposit native tokens to bft bridge, to pay fee.
    let wrapped_evm_client = ctx.context.evm_client(ADMIN);
    let bob_id = Id256::from_evm_address(&ctx.bob_address, CHAIN_ID as _);
    ctx.context
        .native_token_deposit(
            &wrapped_evm_client,
            ctx.fee_charge_address.clone(),
            &ctx.bob_wallet,
            &[bob_id],
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
            &ctx.base_bft_bridge,
            amount,
            Some(memo),
        )
        .await
        .unwrap();

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

    // Let us try fetching the operation by memo
    let operations = erc20_bridge_client
        .get_operations_by_memo(memo)
        .await
        .unwrap();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].0, alice_address);
    assert_eq!(operations[0].1.as_u64(), expected_operation_id as u64);

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
        .get_operations_list(&alice_address, None)
        .await
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    assert!(operation.1.is_complete());
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

    // spender should deposit native tokens to bft bridge, to pay fee.
    let native_balance_after_deposit = 10_u64.pow(15);
    let bob_id = Id256::from_evm_address(&ctx.bob_address, CHAIN_ID as _);
    let init_native_balance = ctx
        .context
        .native_token_deposit(
            &wrapped_evm_client,
            ctx.fee_charge_address.clone(),
            &ctx.bob_wallet,
            &[bob_id],
            native_balance_after_deposit.into(),
        )
        .await
        .unwrap();
    assert_eq!(init_native_balance.0.as_u64(), native_balance_after_deposit);

    let queried_balance = ctx
        .context
        .native_token_deposit_balance(
            &wrapped_evm_client,
            ctx.fee_charge_address.clone(),
            ctx.bob_address(),
        )
        .await;
    assert_eq!(queried_balance.0.as_u64(), native_balance_after_deposit);

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
            &ctx.base_bft_bridge,
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

    let erc20_bridge_client = ctx.context.erc_bridge_client(ADMIN);

    let mint_op = erc20_bridge_client
        .get_operations_list(&alice_address, None)
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
        .get_operations_list(&alice_address, None)
        .await
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    assert!(operation.1.is_complete());

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

    // spender should deposit native tokens to bft bridge, to pay fee.
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
            &ctx.base_bft_bridge,
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
    let erc20_bridge_client = ctx.context.erc_bridge_client(ADMIN);
    let operation = erc20_bridge_client
        .get_operations_list(&alice_address, None)
        .await
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    let signed_order = operation.1.stage.get_signed_mint_order().unwrap();

    ctx.context
        .mint_erc_20_with_order(&ctx.bob_wallet, &ctx.wrapped_bft_bridge, *signed_order)
        .await
        .unwrap();

    ctx.context
        .advance_by_times(Duration::from_secs(2), 10)
        .await;

    // check the operation is complete after the successful mint
    let operation = erc20_bridge_client
        .get_operations_list(&alice_address, None)
        .await
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    assert!(operation.1.is_complete());

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

    // Deposit native tokens to bft bridge.
    let native_token_deposit = 10_000_000_u64;
    let wrapped_evm_client = ctx.context.evm_client(ADMIN);
    let bob_id = Id256::from_evm_address(&ctx.bob_address, CHAIN_ID as _);
    ctx.context
        .native_token_deposit(
            &wrapped_evm_client,
            ctx.fee_charge_address.clone(),
            &ctx.bob_wallet,
            &[bob_id],
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

async fn create_bft_bridge(
    ctx: &PocketIcTestContext,
    wallet: &Wallet<'static, SigningKey>,
    side: BridgeSide,
    fee_charge: H160,
    minter_address: H160,
) -> H160 {
    let is_wrapped = match side {
        BridgeSide::Base => false,
        BridgeSide::Wrapped => true,
    };

    let mut bft_input = BFTBridge::BYTECODE.to_vec();
    let constructor = BFTBridge::constructorCall {}.abi_encode();
    bft_input.extend_from_slice(&constructor);

    let evm = match side {
        BridgeSide::Base => ctx.canisters().external_evm(),
        BridgeSide::Wrapped => ctx.canisters().evm(),
    };

    let evm_client = EvmCanisterClient::new(ctx.client(evm, ADMIN));

    let bridge_address = ctx
        .create_contract_on_evm(&evm_client, wallet, bft_input.clone())
        .await
        .unwrap();

    let init_data = BFTBridge::initializeCall {
        minterAddress: minter_address.into(),
        feeChargeAddress: fee_charge.into(),
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
