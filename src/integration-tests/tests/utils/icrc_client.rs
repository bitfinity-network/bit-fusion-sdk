use candid::Nat;
use ic_canister_client::CanisterClient;
use ic_exports::icrc_types::icrc1::account::Account;
use ic_exports::icrc_types::icrc1::transfer::{TransferArg, TransferError};
use ic_exports::icrc_types::icrc2::approve::ApproveArgs;

use super::error::Result;

pub struct IcrcClient<C: CanisterClient>(C);

impl<C: CanisterClient> IcrcClient<C> {
    pub fn new(client: C) -> Self {
        Self(client)
    }

    /// Transfers ICRC-1 tokens.
    pub async fn icrc1_transfer(&self, to: Account, amount: Nat) -> Result<Nat> {
        let transfer_args = TransferArg {
            from_subaccount: None,
            to,
            fee: None,
            created_at_time: None,
            memo: None,
            amount,
        };

        let res = self
            .0
            .update::<_, std::result::Result<Nat, TransferError>>(
                "icrc1_transfer",
                (transfer_args,),
            )
            .await??;

        Ok(res)
    }

    /// Approves icrc2 transfer.
    pub async fn icrc2_approve(&self, spender: Account, amount: Nat) -> Result<Nat> {
        let approve_args = ApproveArgs {
            from_subaccount: None,
            spender,
            amount,
            expected_allowance: None,
            expires_at: None,
            fee: None,
            memo: None,
            created_at_time: None,
        };

        let res = self
            .0
            .update::<_, std::result::Result<Nat, TransferError>>("icrc2_approve", (approve_args,))
            .await??;

        Ok(res)
    }

    /// Returns the balance of an ICRC token account.
    pub async fn icrc1_balance_of(&self, acc: Account) -> Result<Nat> {
        Ok(self.0.query("icrc1_balance_of", (acc,)).await?)
    }
}
