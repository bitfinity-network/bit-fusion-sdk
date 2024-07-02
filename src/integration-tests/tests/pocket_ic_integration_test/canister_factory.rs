use candid::Principal;
use canister_factory::canister::CanisterFactoryClient;
use canister_factory::types::CanisterArgs;
use erc20_minter::state::Settings;
use ic_log::LogSettings;
use icrc2_minter::SigningStrategy;
use minter_contract_utils::evm_link::EvmLink;

use crate::context::bridge_client::BridgeCanisterClient;
use crate::context::erc20_bridge_client::Erc20BridgeClient;
use crate::context::{CanisterType, TestContext};
use crate::pocket_ic_integration_test::PocketIcTestContext;

#[tokio::test]
async fn test_canister_creation_with_factory_canister() {
    let ctx = PocketIcTestContext::new(&[CanisterType::CanisterFactory]).await;
    let client = ctx.new_client(ctx.canisters.canister_factory(), ctx.admin_name());

    let factory_client = CanisterFactoryClient::new(client);

    let init_data = erc20_minter::state::Settings {
        base_evm_link: EvmLink::Ic(Principal::anonymous()),
        wrapped_evm_link: EvmLink::Ic(Principal::anonymous()),
        signing_strategy: SigningStrategy::Local {
            private_key: rand::random(),
        },
        log_settings: Some(LogSettings {
            enable_console: true,
            in_memory_records: None,
            log_filter: Some("trace".to_string()),
        }),
    };

    let canister_id = factory_client
        .deploy(
            CanisterArgs::ERC20(init_data),
            CanisterType::CkErc20Minter.default_canister_wasm().await,
            None,
            None,
        )
        .await
        .unwrap()
        .unwrap();

    // Check that the canister was created
    let info = factory_client
        .get_canister_info(canister_id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        canister_factory::types::CanisterType::ERC20,
        info.canister_type
    );

    assert!(info.status.is_deployed());

    let erc_client = ctx.new_client(ctx.canisters.canister_factory(), ctx.admin_name());

    let erc_client = Erc20BridgeClient::new(erc_client);

    let owner = erc_client.get_owner().await.unwrap();

    assert_eq!(owner, ctx.admin());
}
