use candid::{Nat, Principal};
use ic_canister::virtual_canister_call;
use ic_exports::ic_kit::RejectionCode;
use ic_exports::icrc_types::icrc1::account::{Account, Subaccount};
use ic_exports::icrc_types::icrc1::transfer::{TransferArg, TransferError};

pub struct CkBtcLedgerClient(Principal);

impl From<Principal> for CkBtcLedgerClient {
    fn from(principal: Principal) -> Self {
        Self(principal)
    }
}

impl CkBtcLedgerClient {
    pub async fn icrc1_balance_of(&self, account: Account) -> Result<Nat, (RejectionCode, String)> {
        virtual_canister_call!(self.0, "icrc1_balance_of", (account,), Nat).await
    }

    pub async fn icrc1_transfer(
        &self,
        to: Account,
        amount: Nat,
        fee: Nat,
        from_subaccount: Option<Subaccount>,
    ) -> Result<Result<Nat, TransferError>, (RejectionCode, String)> {
        let args = TransferArg {
            from_subaccount,
            to,
            fee: Some(fee),
            created_at_time: None,
            memo: None,
            amount,
        };
        virtual_canister_call!(self.0, "icrc1_transfer", (args,), Result<Nat, TransferError>).await
    }
}
