use candid::utils::ArgumentEncoder;
use candid::{encode_args, CandidType, Encode, Principal};
use ic_canister::{init, query, Canister, MethodType, PreUpdate};
use ic_exports::ic_cdk::api::management_canister::main::{
    CanisterIdRecord, CanisterInstallMode, CanisterStatusType, CreateCanisterArgument,
    InstallCodeArgument,
};

use minter_did::init::InitData;

use crate::core::{CanisterId, Management};

use ic_exports::ic_cdk::api::management_canister::provisional::CanisterSettings;

pub const CREATE_CYCLES: u128 = 2_000_000_000;

pub struct CanisterConfig {
    pub controllers: Vec<Principal>,
}

// We keep track of the canister ID of the ICRC canister
pub struct Canisters {
    // pub store:
}

#[derive(CandidType, Clone)]
pub enum CanisterType {
    ICRC(Option<InitData>),
    ERC20(Option<erc20_minter::state::Settings>),
}

/// A canister to transfer funds between IC token canisters and EVM canister contracts.
#[derive(Canister, Clone)]
pub struct UpgraderCanister {
    #[id]
    id: Principal,
}

pub struct UpgraderInitData {
    /// The principal which will be the owner of the newly created canister.
    pub owner: Principal,
}

impl PreUpdate for UpgraderCanister {
    fn pre_update(&self, _method_name: &str, _method_type: MethodType) {}
}

impl UpgraderCanister {
    // #[init]
    fn init(&self, init: UpgraderInitData) {}

    // #[query]
    pub fn validate(&self, canister_type: &CanisterType, wasm: Vec<u8>) {
        let canister_type = match canister_type {
            CanisterType::ICRC(_) => icrc2_minter::ICRC_CANISTER_MARKER,
            CanisterType::ERC20(_) => erc20_minter::ERC20_CANISTER_MARKER,
        };

        let marker_is_valid = wasm
            .windows(canister_type.len())
            .any(|window| window == canister_type.as_bytes());

        if !marker_is_valid {
            panic!("Invalid canister marker");
        }
    }

    /// Deploy Contract
    pub async fn deploy(
        &self,
        canister_type: CanisterType,
        wasm: Vec<u8>,
        config: CanisterConfig,
    ) -> Principal {
        self.validate(&canister_type, wasm.clone());
        let args = CreateCanisterArgument {
            settings: Some(CanisterSettings {
                controllers: Some(config.controllers),
                compute_allocation: None,
                memory_allocation: None,
                freezing_threshold: None,
                reserved_cycles_limit: None,
            }),
        };

        // Create empty canister
        let canister_id = Management::create_canister(args, CREATE_CYCLES)
            .await
            .unwrap();

        match canister_type {
            CanisterType::ICRC(args) => {
                let Some(args) = args else {
                    panic!("ICRC canister requires init data")
                };

                Self::deploy_canister(canister_id, wasm, (args,)).await
            }
            CanisterType::ERC20(settings) => {
                let Some(settings) = settings else {
                    panic!("ERC20 canister requires settings")
                };

                Self::deploy_canister(canister_id, wasm, settings).await
            }
        }
        .unwrap();

        canister_id.canister_id
    }

    pub async fn upgrade(&self, canister_type: CanisterType, principal: Principal, wasm: Vec<u8>) {
        self.validate(&canister_type, wasm.clone());

        let arg: Vec<u8> = encode_args(()).unwrap();
        let arg = InstallCodeArgument {
            mode: CanisterInstallMode::Upgrade(None),
            canister_id: principal,
            wasm_module: wasm,
            arg,
        };
        Management::install_code(arg).await.unwrap();
    }

    pub async fn reinstall(
        &self,
        canister_type: CanisterType,
        principal: Principal,
        wasm: Vec<u8>,
    ) {
        self.validate(&canister_type, wasm.clone());

        let arg = match canister_type {
            CanisterType::ICRC(args) => {
                let Some(args) = args else {
                    panic!("ICRC canister requires init data")
                };

                Encode!(&args).unwrap()
            }
            CanisterType::ERC20(settings) => {
                let Some(settings) = settings else {
                    panic!("ERC20 canister requires settings")
                };

                Encode!(&settings).unwrap()
            }
        };

        let arg = InstallCodeArgument {
            mode: CanisterInstallMode::Reinstall,
            canister_id: principal,
            wasm_module: wasm,
            arg,
        };
        Management::install_code(arg).await.unwrap();
    }

    async fn deploy_canister<T: CandidType + Send>(
        canister_id: CanisterIdRecord,
        wasm: Vec<u8>,
        args: T,
    ) -> Result<(), String> {
        let encoded_args = Encode!(&args).unwrap();
        let arg = InstallCodeArgument {
            mode: CanisterInstallMode::Install,
            canister_id: canister_id.canister_id,
            wasm_module: wasm,
            arg: encoded_args,
        };
        Management::install_code(arg).await.unwrap();

        // Assert that the canister was created successfully
        let status = Management::canister_status(canister_id.canister_id)
            .await
            .unwrap();

        //
        match status.status {
            CanisterStatusType::Running => Ok(()),
            _ => Err("Canister not running".to_string()),
        }
    }

    // add new implementation for bft
}
