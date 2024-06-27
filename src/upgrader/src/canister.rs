use std::cell::RefCell;
use std::rc::Rc;

use candid::utils::ArgumentEncoder;
use candid::{encode_args, CandidType, Encode, Principal};
use did::H160;
use ic_canister::{init, query, update, Canister, MethodType, PreUpdate};
use ic_exports::ic_cdk::api::management_canister::main::{
    CanisterIdRecord, CanisterInstallMode, CanisterStatusType, CreateCanisterArgument,
    InstallCodeArgument,
};

use ic_exports::ic_kit::ic;
use ic_log::LogSettings;
use icrc2_minter::SigningStrategy;
use minter_did::init::InitData;

use crate::core::{CanisterId, Management};
use crate::state::{Settings, State};

use ic_exports::ic_cdk::api::management_canister::provisional::CanisterSettings;

use eth_signer::sign_strategy::TransactionSigner;

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
    pub signing_strategy: SigningStrategy,
}

impl PreUpdate for UpgraderCanister {
    fn pre_update(&self, _method_name: &str, _method_type: MethodType) {}
}

impl UpgraderCanister {
    // #[init]
    fn init(&self, init: UpgraderInitData) {
        let state = get_state();
        let mut state = state.borrow_mut();

        check_anonymous_principal(init.owner).expect("anonymous principal not allowed");

        let settings = Settings {
            owner: init.owner,
            signing_strategy: init.signing_strategy,
        };

        state.reset(settings);
    }

    /// Returns principal of canister owner.
    #[query]
    pub fn get_owner(&self) -> Principal {
        get_state().borrow().config.get_owner()
    }

    /// set_owner inspect_message check
    pub(crate) fn set_owner_inspect_message_check(
        principal: Principal,
        owner: Principal,
        state: &State,
    ) -> Result<(), ()> {
        check_anonymous_principal(owner).unwrap();
        inspect_check_is_owner(principal, state).unwrap();

        Ok(())
    }

    /// Sets a new principal for canister owner.
    ///
    /// This method should be called only by current owner,
    /// else `Error::NotAuthorised` will be returned.
    #[update]
    pub fn set_owner(&mut self, owner: Principal) -> Result<(), String> {
        let state = get_state();
        let mut state = state.borrow_mut();

        UpgraderCanister::set_owner_inspect_message_check(ic::caller(), owner, &state).unwrap();
        state.config.set_owner(owner);

        Ok(())
    }

    /// Returns evm_address of the minter canister.
    #[update]
    pub async fn get_canister_evm_address(&mut self) -> Result<H160, ()> {
        let signer = get_state().borrow().signer.get_transaction_signer();
        Ok(signer.get_address().await.unwrap())
    }

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

thread_local! {
    pub static STATE: Rc<RefCell<State>> = Rc::default();
}

pub(crate) fn get_state() -> Rc<RefCell<State>> {
    STATE.with(|state| state.clone())
}

/// inspect function to check whether provided principal is owner
fn inspect_check_is_owner(principal: Principal, state: &State) -> Result<(), ()> {
    let owner = state.config.get_owner();

    // if owner != principal {
    //     return Err(Error::NotAuthorized);
    // }

    todo!()
}

/// inspect function to check whether the provided principal is anonymous
fn check_anonymous_principal(principal: Principal) -> Result<(), ()> {
    // if principal == Principal::anonymous() {
    //     return Err(Error::AnonymousPrincipal);
    // }

    todo!()
}
