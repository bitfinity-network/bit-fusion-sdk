use std::time::Duration;

use candid::{Nat, Principal};
use did::{H160, U256, U64};
use erc20_minter::client::EvmLink;
use erc20_minter::state::Settings;
use eth_signer::{Signer, Wallet};
use ethers_core::abi::{Constructor, Param, ParamType, Token};
use ethers_core::k256::ecdsa::SigningKey;
use evm_canister_client::EvmCanisterClient;
use ic_canister_client::CanisterClientError;
use ic_exports::ic_kit::mock_principals::{alice, john};
use ic_exports::icrc_types::icrc2::transfer_from::TransferFromError;
use ic_exports::pocket_ic::{CallError, ErrorCode, UserError};
use ic_log::LogSettings;
use icrc2_minter::tokens::icrc1::IcrcTransferDst;
use icrc2_minter::SigningStrategy;
use minter_contract_utils::bft_bridge_api::BURN;
use minter_contract_utils::build_data::test_contracts::{
    BFT_BRIDGE_SMART_CONTRACT_CODE, TEST_WTM_HEX_CODE, WRAPPED_TOKEN_SMART_CONTRACT_CODE,
};
use minter_contract_utils::{bft_bridge_api, wrapped_token_api, BridgeSide};
use minter_did::error::Error as McError;
use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;

use super::{PocketIcTestContext, JOHN};
use crate::context::{
    evm_canister_init_data, CanisterType, TestContext, ICRC1_INITIAL_BALANCE, ICRC1_TRANSFER_FEE,
};
use crate::pocket_ic_integration_test::{ADMIN, ALICE};
use crate::utils::error::TestError;
use crate::utils::{self, CHAIN_ID};

#[tokio::test]
async fn set_owner_access() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Minter]).await;
    let mut admin_client = ctx.minter_client(ADMIN);
    admin_client.set_owner(alice()).await.unwrap().unwrap();

    // Now Alice is owner, so admin can't update owner anymore.
    let err = admin_client.set_owner(alice()).await.unwrap_err();
    assert!(matches!(
        err,
        CanisterClientError::PocketIcTestError(CallError::UserError(UserError {
            code: ErrorCode::CanisterCalledTrap,
            description: _,
        }))
    ));

    // Now Alice is owner, so she can update owner.
    let mut alice_client = ctx.minter_client(ALICE);
    alice_client.set_owner(alice()).await.unwrap().unwrap();
}

#[tokio::test]
async fn invalid_bridge_contract() {
    let ctx = PocketIcTestContext::new(&CanisterType::MINTER_TEST_SET).await;
    let minter_client = ctx.minter_client(ADMIN);
    let res = minter_client
        .register_evmc_bft_bridge(H160::from_slice(&[20; 20]))
        .await
        .unwrap()
        .unwrap_err();

    assert_eq!(res, McError::InvalidBftBridgeContract);
}

#[tokio::test]
async fn invalid_bridge() {
    let ctx = PocketIcTestContext::new(&CanisterType::MINTER_TEST_SET).await;
    let admin = ADMIN;
    let admin_wallet = ctx.new_wallet(u128::MAX).await.unwrap();
    let minter_canister_address = ctx.get_minter_canister_evm_address(admin).await.unwrap();

    let contract_code = WRAPPED_TOKEN_SMART_CONTRACT_CODE.clone();
    let input = wrapped_token_api::CONSTRUCTOR
        .encode_input(
            contract_code,
            &[
                Token::String("name".into()),
                Token::String("symbol".into()),
                Token::Address(minter_canister_address.into()),
            ],
        )
        .unwrap();
    let contract = ctx.create_contract(&admin_wallet, input).await.unwrap();

    let minter_client = ctx.minter_client(ADMIN);
    let res = minter_client
        .register_evmc_bft_bridge(contract)
        .await
        .unwrap()
        .unwrap_err();

    assert_eq!(res, McError::InvalidBftBridgeContract);
}

#[tokio::test]
async fn double_register_bridge() {
    let ctx = PocketIcTestContext::new(&CanisterType::MINTER_TEST_SET).await;
    let admin_wallet = ctx.new_wallet(u128::MAX).await.unwrap();

    let _ = ctx
        .initialize_bft_bridge(ADMIN, &admin_wallet)
        .await
        .unwrap();
    let err = ctx
        .initialize_bft_bridge(ADMIN, &admin_wallet)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        TestError::CanisterClient(CanisterClientError::PocketIcTestError(
            CallError::UserError(UserError {
                code: ErrorCode::CanisterCalledTrap,
                description: _,
            })
        ))
    ));
}

#[tokio::test]
async fn canister_log_config_should_still_be_storable_after_upgrade() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Minter]).await;

    let minter_client = ctx.minter_client(ADMIN);

    assert!(minter_client
        .set_logger_filter("info".to_string())
        .await
        .unwrap()
        .is_ok());

    // Advance state to avoid canister rate limit.
    for _ in 0..100 {
        ctx.client.tick().await;
    }

    // upgrade canister
    ctx.upgrade_minter_canister().await.unwrap();
    assert!(minter_client
        .set_logger_filter("debug".to_string())
        .await
        .unwrap()
        .is_ok());
}

#[tokio::test]
async fn test_canister_build_data() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Minter]).await;
    let minter_client = ctx.minter_client(ALICE);
    let build_data = minter_client.get_canister_build_data().await.unwrap();
    assert!(build_data.pkg_name.contains("icrc2-minter"));
}
