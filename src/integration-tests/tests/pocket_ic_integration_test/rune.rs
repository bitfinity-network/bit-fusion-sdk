use std::collections::HashSet;
use std::time::Duration;

use bridge_client::RuneBridgeClient;
use bridge_did::init::{BridgeInitData, RuneBridgeConfig};
use candid::Principal;
use did::H160;
use eth_signer::sign_strategy::{SigningKeyId, SigningStrategy};
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_exports::ic_kit::mock_principals::bob;


use crate::context::TestContext;
use crate::utils::wasm::get_rune_bridge_canister_bytecode;

use super::PocketIcTestContext;

const KEY_ID: &str = "test_key_1";

struct RunesSetup {
    ctx: PocketIcTestContext,
    rune_bridge: Principal,
}

impl RunesSetup {
    async fn init() -> RunesSetup {
        let context = PocketIcTestContext::new(&[]).await;

        let bridge = (&context).create_canister().await.unwrap();
        let init_args = BridgeInitData {
            evm_principal: bob(),
            signing_strategy: SigningStrategy::ManagementCanister {
                key_id: SigningKeyId::Custom(KEY_ID.to_string()),
            },
            owner: (&context).admin(),
            log_settings: Default::default(),
        };
        let rune_config = RuneBridgeConfig {
            network: BitcoinNetwork::Mainnet,
            min_confirmations: 1,
            indexer_urls: HashSet::from_iter(["https://indexer".to_string()]),
            deposit_fee: 0,
            mempool_timeout: Duration::from_secs(60),
            indexer_consensus_threshold: 1,
        };
        (&context)
            .install_canister(
                bridge,
                get_rune_bridge_canister_bytecode().await,
                (init_args, rune_config),
            )
            .await
            .unwrap();

        let rune_bridge_client = RuneBridgeClient::new(context.client(bridge, "admin"));
        rune_bridge_client.admin_configure_ecdsa().await.unwrap();

        RunesSetup {
            ctx: context,
            rune_bridge: bridge,
        }
    }

}

#[tokio::test]
async fn generates_correct_deposit_address() {
    
    const ETH_ADDRESS: &str = "0x4e37fc8684e0f7ad6a6c1178855450294a16b418";
    let eth_address = H160::from_hex_str(ETH_ADDRESS).unwrap();
    
    let setup = RunesSetup::init().await;
    let rune_bridge_client = RuneBridgeClient::new(setup.ctx.client(setup.rune_bridge, "alice"));
    let address = rune_bridge_client.get_deposit_address(&eth_address).await.unwrap().unwrap();

    let expected = "bc1qq9c8dh2w7vk25644y3ulf808vyggx9z8c6tapp".to_string();

    assert_eq!(
        address,
        expected
    );

    const ANOTHER_ETH_ADDRESS: &str = "0x4e37fc8684e0f7ad6a6c1178855450294a16b419";
    let eth_address = H160::from_hex_str(ANOTHER_ETH_ADDRESS).unwrap();

    let address = rune_bridge_client.get_deposit_address(&eth_address).await.unwrap().unwrap();

    assert_ne!(
        address,
        expected
    );

}
