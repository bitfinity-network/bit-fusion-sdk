use bridge_canister::runtime::RuntimeState;
use bridge_did::error::{BftResult, Error};
use candid::{CandidType, Deserialize};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use serde::Serialize;

use super::Brc20BridgeOp;
use crate::core::withdrawal::{Brc20WithdrawalPayload, DidTransaction, Withdrawal};
use crate::{state, Brc20Bridge};

/// BRC20 bridge withdraw operations
#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum Brc20BridgeWithdrawOp {
    /// Create BRC20 transfer inscription transactions
    CreateInscriptionTxs(Brc20WithdrawalPayload),
    /// Send BRC20 transfer inscription transactions
    SendInscriptionTxs {
        payload: Brc20WithdrawalPayload,
        commit_tx: DidTransaction,
        reveal_tx: DidTransaction,
        reveal_utxo: Utxo,
    },
    /// Await for the BRC20 transfer inscription transactions to be confirmed
    AwaitInscriptionTxs {
        payload: Brc20WithdrawalPayload,
        reveal_utxo: Utxo,
    },
    /// Create transfer transaction
    CreateTransferTx {
        payload: Brc20WithdrawalPayload,
        reveal_utxo: Utxo,
    },
    /// Send transfer transaction
    SendTransferTx {
        from_address: H160,
        tx: DidTransaction,
    },
    /// Transfer transaction sent
    TransferTxSent {
        from_address: H160,
        tx: DidTransaction,
    },
}

impl Brc20BridgeWithdrawOp {
    /// Create BRC20 transfer inscription transactions
    pub async fn create_inscription_txs(
        state: RuntimeState<Brc20BridgeOp>,
        payload: Brc20WithdrawalPayload,
    ) -> BftResult<Brc20BridgeOp> {
        let withdraw = Withdrawal::get()
            .map_err(|err| Error::FailedToProgress(format!("cannot get withdraw: {err:?}")))?;
        todo!();
    }
}
