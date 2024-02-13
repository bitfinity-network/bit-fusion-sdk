use std::cell::RefCell;
use std::rc::Rc;

use candid::{Nat, Principal};
use did::ic::StorablePrincipal;
use ic_canister::{
    generate_idl, init, post_upgrade, update, virtual_canister_call, Canister, Idl, PreUpdate,
};
use ic_exports::ic_kit::ic;
use ic_exports::icrc_types::icrc1::account::Subaccount;
use ic_exports::icrc_types::icrc2::transfer_from::{TransferFromArgs, TransferFromError};
use ic_metrics::{Metrics, MetricsStorage};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, MemoryId, StableCell, VirtualMemory};

use crate::memory::MEMORY_MANAGER;

#[derive(Canister, Clone, Debug)]
pub struct SpenderCanister {
    #[id]
    id: Principal,
}

impl PreUpdate for SpenderCanister {}

impl SpenderCanister {
    fn set_timers(&mut self) {
        // Set the metrics updating interval
        #[cfg(target_family = "wasm")]
        {
            self.update_metrics_timer(std::time::Duration::from_secs(60 * 60));
        }
    }
    #[init]
    pub fn init(&mut self, minter_canister_principal: Principal) {
        self.set_timers();
        MINTER_CANISTER_PRINCIPAL_CELL.with(|cell| {
            cell.borrow_mut()
                .set(StorablePrincipal(minter_canister_principal))
                .expect("failed to set minter_canister_principal to stable memory")
        });
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.set_timers();
    }

    /// Performs `transferFrom` from minter canister to the recipient of the `burn_tx`.
    /// The transfer should be approved with `MinterCanister::start_icrc2_mint()` call.
    ///
    /// Can be called only by minter canister.
    #[update]
    pub async fn finish_icrc2_mint(
        &self,
        token: Principal,
        recipient: Principal,
        spender_subaccount: Subaccount,
        amount: Nat,
        fee: Nat,
    ) -> Result<Nat, TransferFromError> {
        let minter_canister = minter_canister();
        if ic::caller() != minter_canister {
            return Err(TransferFromError::GenericError {
                error_code: 0xff_u64.into(),
                message: "only minter canister can call `SpenderCanister::finish_icrc2_mint`"
                    .into(),
            });
        }

        let args = TransferFromArgs {
            spender_subaccount: Some(spender_subaccount),
            from: minter_canister.into(),
            to: recipient.into(),
            amount,
            fee: Some(fee),
            memo: None,
            created_at_time: None,
        };

        virtual_canister_call!(token, "icrc2_transfer_from", (args,), std::result::Result<Nat, TransferFromError>)
        .await.unwrap_or_else(|e| {
            Err(TransferFromError::GenericError {
                error_code: (e.0 as u64).into(),
                message: e.1,
            })
        })
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}

impl Metrics for SpenderCanister {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}

const MINTER_CANISTER_PRINCIPAL_MEMORY_ID: MemoryId = MemoryId::new(0);

thread_local! {
    static MINTER_CANISTER_PRINCIPAL_CELL: RefCell<StableCell<StorablePrincipal, VirtualMemory<DefaultMemoryImpl>>> = {
        RefCell::new(StableCell::new(MEMORY_MANAGER.with(|mm| mm.get(MINTER_CANISTER_PRINCIPAL_MEMORY_ID)), StorablePrincipal(Principal::anonymous()))
            .expect("stable memory minter_canister_principal initialization failed"))
    };
}

fn minter_canister() -> Principal {
    MINTER_CANISTER_PRINCIPAL_CELL.with(|cell| cell.borrow().get().0)
}

#[cfg(test)]
mod test {}
