use candid::Principal;
use ic_canister::virtual_canister_call;
use ic_exports::ic_kit::RejectionCode;
use ic_exports::ledger::Subaccount;

use super::interface::{RetrieveBtcArgs, RetrieveBtcError, RetrieveBtcOk};
use super::{UpdateBalanceArgs, UpdateBalanceError, UtxoStatus};

pub struct CkBtcMinterClient(Principal);

impl From<Principal> for CkBtcMinterClient {
    fn from(principal: Principal) -> Self {
        Self(principal)
    }
}

impl CkBtcMinterClient {
    /// Send an update request to ckBTC minter to check for new UTXOs and mint them as ckBTC tokens.
    /// The function returns the mint status for each found UTXO.
    ///
    /// For more details, see [update_balance](https://internetcomputer.org/docs/current/references/ckbtc-reference#update_balanceowner-opt-principal-subaccount-opt-blob).
    pub async fn update_balance(
        &self,
        owner: Principal,
        subaccount: Option<Subaccount>,
    ) -> Result<Result<Vec<UtxoStatus>, UpdateBalanceError>, (RejectionCode, String)> {
        let args = UpdateBalanceArgs {
            owner: Some(owner),
            subaccount,
        };

        virtual_canister_call!(
            self.0,
            "update_balance",
            (args,),
            Result<Vec<UtxoStatus>, UpdateBalanceError>
        )
        .await
    }

    pub async fn retrieve_btc(
        &self,
        address: String,
        amount: u64,
    ) -> Result<Result<RetrieveBtcOk, RetrieveBtcError>, (RejectionCode, String)> {
        let args = RetrieveBtcArgs { address, amount };

        virtual_canister_call!(
            self.0,
            "retrieve_btc",
            (args,),
            Result<RetrieveBtcOk, RetrieveBtcError>
        )
        .await
    }
}
