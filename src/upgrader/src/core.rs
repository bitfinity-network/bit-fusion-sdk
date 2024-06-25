use candid::{CandidType, Principal};
use ic_exports::ic_cdk::api::{
    call::{call, call_with_payment, call_with_payment128},
    management_canister::{
        main::{
            CanisterInfoRequest, CanisterInfoResponse, CanisterStatusResponse,
            CreateCanisterArgument, InstallCodeArgument, UpdateSettingsArgument,
        },
        provisional::CanisterIdRecord,
    },
};
use serde::de::DeserializeOwned;

pub type CanisterId = Principal;

pub enum CallCycles {
    Cycles(u128),
    Free,
}

pub const MANAGEMENT_CANISTER_ID: Principal = Principal::management_canister();

pub struct Management;

impl Management {
    pub async fn call<A, R>(method: &str, args: A, cycles: CallCycles) -> Result<R, String>
    where
        A: CandidType,
        R: CandidType + DeserializeOwned,
    {
        let res: Result<(R,), _> = match cycles {
            CallCycles::Cycles(cycles) => {
                call_with_payment128(MANAGEMENT_CANISTER_ID, method, (args,), cycles).await
            }
            CallCycles::Free => call(MANAGEMENT_CANISTER_ID, method, (args,)).await,
        };

        match res {
            Ok((res,)) => Ok(res),
            Err(e) => Err(String::from(format!("{} {}", method.to_string(), e.1))),
        }
    }

    pub async fn create_canister(
        arg: CreateCanisterArgument,
        cycles: u128,
    ) -> Result<CanisterIdRecord, String> {
        Management::call("create_canister", arg, CallCycles::Cycles(cycles)).await
    }

    pub async fn install_code(arg: InstallCodeArgument) -> Result<(), String> {
        Management::call("install_code", arg, CallCycles::Free).await
    }

    pub async fn update_settings(arg: UpdateSettingsArgument) -> Result<(), String> {
        Management::call("update_settings", arg, CallCycles::Free).await
    }

    pub async fn canister_status(
        canister_id: CanisterId,
    ) -> Result<CanisterStatusResponse, String> {
        let arg = CanisterIdRecord { canister_id };

        Management::call("canister_status", arg, CallCycles::Free).await
    }

    pub async fn start_canister(canister_id: CanisterId) -> Result<(), String> {
        let arg = CanisterIdRecord { canister_id };

        Management::call("start_canister", arg, CallCycles::Free).await
    }

    pub async fn stop_canister(canister_id: CanisterId) -> Result<(), String> {
        let arg = CanisterIdRecord { canister_id };

        Management::call("stop_canister", arg, CallCycles::Free).await
    }

    pub async fn delete_canister(canister_id: CanisterId) -> Result<(), String> {
        let arg = CanisterIdRecord { canister_id };

        Management::call("delete_canister", arg, CallCycles::Free).await
    }

    pub async fn uninstall_code(canister_id: CanisterId) -> Result<(), String> {
        let arg = CanisterIdRecord { canister_id };

        Management::call("uninstall_code", arg, CallCycles::Free).await
    }

    pub async fn deposit_cycles(canister_id: CanisterId, cycles: u128) -> Result<(), String> {
        let arg = CanisterIdRecord { canister_id };

        Management::call("deposit_cycles", arg, CallCycles::Cycles(cycles)).await
    }

    pub async fn raw_rand() -> Result<Vec<u8>, String> {
        Management::call("raw_rand", (), CallCycles::Free).await
    }

    pub async fn canister_info(
        canister_id: CanisterId,
        num_requested_changes: Option<u64>,
    ) -> Result<CanisterInfoResponse, String> {
        let arg = CanisterInfoRequest {
            canister_id,
            num_requested_changes,
        };

        Management::call("canister_info", arg, CallCycles::Free).await
    }
}
