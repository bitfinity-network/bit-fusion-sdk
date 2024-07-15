use std::time::Duration;

use alloy_sol_types::SolConstructor;
use bridge_did::error::BftResult;
use bridge_did::id256::Id256;
use bridge_utils::evm_bridge::BridgeSide;
use did::{H160, H256, U256, U64};
use eth_signer::{Signer, Wallet};
use ethers_core::k256::ecdsa::SigningKey;
use evm_canister_client::EvmCanisterClient;
use ic_canister_client::CanisterClient as _;

use super::DfxTestContext;
use crate::context::{CanisterType, TestContext};
use crate::dfx_tests::{TestWTM, ADMIN};
use crate::utils::CHAIN_ID;

#[allow(dead_code)]
pub struct ContextWithBridges {
    pub context: DfxTestContext,
    pub bob_wallet: Wallet<'static, SigningKey>,
    pub bob_address: H160,
    pub erc20_minter_address: H160,
    pub base_bft_bridge: H160,
    pub wrapped_bft_bridge: H160,
    pub base_token_address: H160,
    pub wrapped_token_address: H160,
    pub fee_charge_address: H160,
}

impl ContextWithBridges {
    pub async fn new() -> Self {
        let ctx = DfxTestContext::new(&CanisterType::EVM_MINTER_WITH_EVMRPC_TEST_SET).await;

        // Deploy external EVM canister.
        let base_evm = ctx.canisters().external_evm();
        println!("BASE EVM: {}", base_evm);
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
        ctx.advance_time(Duration::from_secs(10)).await;
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

        // get evm minter canister address
        let erc20_minter_client = ctx.client(ctx.canisters().ck_erc20_minter(), ADMIN);
        let erc20_minter_address = erc20_minter_client
            .update::<_, Option<H160>>("get_evm_address", ())
            .await
            .unwrap()
            .unwrap();

        // mint native tokens for the erc20-minter on both EVMs
        println!("Minting native tokens on both EVMs for {erc20_minter_address}");
        ctx.evm_client(ADMIN)
            .mint_native_tokens(erc20_minter_address.clone(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
        base_evm_client
            .mint_native_tokens(erc20_minter_address.clone(), u64::MAX.into())
            .await
            .unwrap()
            .unwrap();
        ctx.advance_time(Duration::from_secs(2)).await;

        // Deploy the BFTBridge contract on the external EVM.
        let base_bft_bridge =
            create_bft_bridge(&ctx, BridgeSide::Base, expected_fee_charge_address.into()).await;
        println!("Base BFT Bridge: {}", base_bft_bridge);
        let wrapped_bft_bridge = create_bft_bridge(
            &ctx,
            BridgeSide::Wrapped,
            expected_fee_charge_address.into(),
        )
        .await;
        println!("Wrapped BFT Bridge: {}", wrapped_bft_bridge);

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
            erc20_minter_address,
            base_bft_bridge,
            wrapped_bft_bridge,
            base_token_address,
            wrapped_token_address,
            fee_charge_address,
        }
    }
}

// Create a second EVM canister (base_evm) instance and create BFTBridge contract on it, // It will play role of external evm
// Create erc20-minter instance, initialized with EvmInfos for both EVM canisters.
// Deploy ERC-20 token on external_evm,
// Deploy Wrapped token on first EVM for the ERC-20 from previous step,
// Approve ERC-20 transfer on behalf of some user in external_evm,
// Call BFTBridge::burn() on behalf of the user in external_evm.
// Wait some time for the erc20-minter see and process it.
// Make sure the tokens minted.
// Make sure SignedMintOrder removed from erc20-minter after some time.
#[tokio::test]
async fn test_should_setup_with_evm_rpc_canister() {
    let _ = ContextWithBridges::new().await;
}

async fn create_bft_bridge(ctx: &DfxTestContext, side: BridgeSide, fee_charge: H160) -> H160 {
    let minter_client = ctx.client(ctx.canisters().ck_erc20_minter(), ADMIN);

    let hash = minter_client
        .update::<_, BftResult<H256>>("init_bft_bridge_contract", (side, fee_charge))
        .await
        .unwrap()
        .unwrap();

    println!("init_bft_bridge_contract {side} hash: {:?}", hash);

    let evm = match side {
        BridgeSide::Base => ctx.canisters().external_evm(),
        BridgeSide::Wrapped => ctx.canisters().evm(),
    };

    let evm_client = EvmCanisterClient::new(ctx.client(evm, ADMIN));

    ctx.wait_transaction_receipt_on_evm(&evm_client, &hash)
        .await
        .unwrap()
        .unwrap()
        .contract_address
        .expect("contract address")
}
