use candid::Principal;
use did::H160;
use eth_signer::sign_strategy::{SigningKeyId, SigningStrategy};
use ic_canister_client::CanisterClient;
use ic_exports::ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use ic_management_canister_types::{EcdsaCurve, EcdsaKeyId};
use ic_state_machine_tests::StateMachineBuilder;

use rune_bridge::interface::GetAddressError;
use rune_bridge::state::{RuneBridgeConfig, RuneInfo};

use crate::context::TestContext;
use crate::state_machine_tests::StateMachineContext;
use crate::utils::wasm::get_rune_bridge_canister_bytecode;

const KEY_ID: &str = "test_key";

struct RunesSetup {
    ctx: StateMachineContext,
    rune_bridge: Principal,
}

fn key_id() -> EcdsaKeyId {
    EcdsaKeyId {
        curve: EcdsaCurve::Secp256k1,
        name: KEY_ID.to_string(),
    }
}

impl RunesSetup {
    async fn init() -> RunesSetup {
        let context = tokio::task::spawn_blocking(move || {
            StateMachineContext::new(StateMachineBuilder::new().with_ecdsa_key(key_id()).build())
        })
        .await
        .unwrap();

        let bridge = (&context).create_canister().await.unwrap();
        let init_args = RuneBridgeConfig {
            network: BitcoinNetwork::Mainnet,
            evm_link: Default::default(),
            signing_strategy: SigningStrategy::ManagementCanister {
                key_id: SigningKeyId::Custom(KEY_ID.to_string()),
            },
            admin: (&context).admin(),
            log_settings: Default::default(),
            min_confirmations: 1,
            rune_info: RuneInfo {
                name: "".to_string(),
                block: 0,
                tx: 0,
            },
            indexer_url: "".to_string(),
            deposit_fee: 0,
        };
        (&context)
            .install_canister(
                bridge,
                get_rune_bridge_canister_bytecode().await,
                (init_args,),
            )
            .await
            .unwrap();
        let _: () = (&context)
            .client(bridge, "admin")
            .update("admin_configure_ecdsa", ())
            .await
            .unwrap();

        RunesSetup {
            ctx: context,
            rune_bridge: bridge,
        }
    }

    fn rune_client(&self) -> impl CanisterClient {
        (&self.ctx).client(self.rune_bridge, "alice")
    }

    async fn deposit_address(&self, eth_address: &H160) -> String {
        self.rune_client()
            .update::<_, Result<String, GetAddressError>>("get_deposit_address", (eth_address,))
            .await
            .expect("failed to send get deposit address request")
            .expect("failed to get deposit address")
    }

    pub async fn async_drop(self) {
        let env = self.ctx.env;
        tokio::task::spawn_blocking(move || {
            drop(env);
        })
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn generates_correct_deposit_address() {
    const ETH_ADDRESS: &str = "0x4e37fc8684e0f7ad6a6c1178855450294a16b418";
    let eth_address = H160::from_hex_str(ETH_ADDRESS).unwrap();

    let setup = RunesSetup::init().await;
    let address = setup.deposit_address(&eth_address).await;

    assert_eq!(
        address,
        "bc1qdmwl446fszfj40wpup4dgq6ezv8l6ajhs2zxyz".to_string()
    );

    const ANOTHER_ETH_ADDRESS: &str = "0x4e37fc8684e0f7ad6a6c1178855450294a16b419";
    let eth_address = H160::from_hex_str(ANOTHER_ETH_ADDRESS).unwrap();

    let address = setup.deposit_address(&eth_address).await;

    assert_ne!(
        address,
        "bc1qdmwl446fszfj40wpup4dgq6ezv8l6ajhs2zxyz".to_string()
    );

    setup.async_drop().await;
}
