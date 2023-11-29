use candid::Nat;
use evm_canister_client::{CanisterClient, CanisterClientResult};
use ic_exports::icrc_types::icrc1::account::{Account, Subaccount};
use ic_exports::icrc_types::icrc1::transfer::{TransferArg, TransferError};
use ic_exports::icrc_types::icrc2::approve::{ApproveArgs, ApproveError};

pub struct Icrc2CanisterClient<C> {
    client: C,
}

impl<C: CanisterClient> Icrc2CanisterClient<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub async fn icrc1_transfer(
        &self,
        to: Account,
        from_subaccount: Option<Subaccount>,
        amount: Nat,
    ) -> CanisterClientResult<Result<Nat, TransferError>> {
        let transfer = TransferArg {
            from_subaccount,
            to: Account {
                owner: to.owner,
                subaccount: to.subaccount,
            },
            amount,
            fee: None,
            memo: None,
            created_at_time: None,
        };

        self.client.update("icrc1_transfer", (transfer,)).await
    }

    pub async fn icrc2_approve(
        &self,
        from_subaccount: Option<Subaccount>,
        spender: Account,
        amount: Nat,
    ) -> CanisterClientResult<Result<Nat, ApproveError>> {
        let approve_args = ApproveArgs {
            from_subaccount,
            spender,
            amount,
            expected_allowance: None,
            expires_at: None,
            fee: None,
            memo: None,
            created_at_time: None,
        };

        self.client.update("icrc2_approve", (&approve_args,)).await
    }
}
