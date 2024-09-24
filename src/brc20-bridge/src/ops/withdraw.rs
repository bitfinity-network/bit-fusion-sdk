use bridge_did::error::{BftResult, Error};
use bridge_did::operations::{
    Brc20BridgeWithdrawOp, Brc20WithdrawalPayload, DidTransaction, RevealUtxo,
};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;

use super::{Brc20BridgeOp, Brc20BridgeOpImpl};
use crate::core::withdrawal::{Brc20Transactions, Withdrawal};

pub struct Brc20BridgeWithdrawOpImpl;

impl Brc20BridgeWithdrawOpImpl {
    /// Create BRC20 transfer inscription transactions
    pub async fn create_inscription_txs(
        payload: Brc20WithdrawalPayload,
    ) -> BftResult<Brc20BridgeOpImpl> {
        let withdraw = Withdrawal::get()
            .map_err(|err| Error::FailedToProgress(format!("cannot get withdraw: {err:?}")))?;

        let Brc20Transactions {
            commit_tx,
            reveal_tx,
            reveal_utxo,
        } = withdraw
            .build_brc20_transfer_transactions(payload.clone())
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!(
                    "cannot build brc20 transfer transactions: {err:?}"
                ))
            })?;

        Ok(
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendCommitTx {
                payload,
                commit_tx: commit_tx.into(),
                reveal_tx: reveal_tx.into(),
                reveal_utxo,
            })
            .into(),
        )
    }

    /// Send BRC20 transfer commit transaction
    pub async fn send_commit_transaction(
        payload: Brc20WithdrawalPayload,
        commit_tx: DidTransaction,
        reveal_tx: DidTransaction,
        reveal_utxo: RevealUtxo,
    ) -> BftResult<Brc20BridgeOpImpl> {
        let withdraw = Withdrawal::get()
            .map_err(|err| Error::FailedToProgress(format!("cannot get withdraw: {err:?}")))?;

        withdraw
            .send_transaction(commit_tx.into())
            .await
            .map_err(|err| Error::FailedToProgress(format!("cannot send commit tx: {err:?}")))?;

        Ok(
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendRevealTx {
                payload,
                reveal_tx,
                reveal_utxo,
            })
            .into(),
        )
    }

    /// Send BRC20 transfer reveal transaction
    pub async fn send_reveal_transaction(
        payload: Brc20WithdrawalPayload,
        reveal_tx: DidTransaction,
        reveal_utxo: RevealUtxo,
    ) -> BftResult<Brc20BridgeOpImpl> {
        let withdraw = Withdrawal::get()
            .map_err(|err| Error::FailedToProgress(format!("cannot get withdraw: {err:?}")))?;

        withdraw
            .send_transaction(reveal_tx.into())
            .await
            .map_err(|err| Error::FailedToProgress(format!("cannot send reveal tx: {err:?}")))?;

        Ok(
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::AwaitInscriptionTxs {
                payload,
                reveal_utxo,
            })
            .into(),
        )
    }

    /// Check whether the inscription transactions are confirmed
    pub async fn await_inscription_transactions(
        payload: Brc20WithdrawalPayload,
        reveal_utxo: RevealUtxo,
    ) -> BftResult<Brc20BridgeOpImpl> {
        let withdraw = Withdrawal::get()
            .map_err(|err| Error::FailedToProgress(format!("cannot get withdraw: {err:?}")))?;

        let reveal_utxo = withdraw
            .await_inscription_transactions(&payload.sender, reveal_utxo)
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!(
                    "failed to await inscription transactions: {err:?}"
                ))
            })?;

        log::debug!("reveal UTXO landed at block {}", reveal_utxo.height);

        Ok(
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::CreateTransferTx {
                payload,
                reveal_utxo,
            })
            .into(),
        )
    }

    /// Create transfer transaction
    pub async fn create_transfer_transaction(
        payload: Brc20WithdrawalPayload,
        reveal_utxo: Utxo,
    ) -> BftResult<Brc20BridgeOpImpl> {
        let withdraw = Withdrawal::get()
            .map_err(|err| Error::FailedToProgress(format!("cannot get withdraw: {err:?}")))?;

        let tx = withdraw
            .build_transfer_transaction(payload.clone(), reveal_utxo)
            .await
            .map_err(|err| Error::FailedToProgress(format!("cannot build transfer tx: {err:?}")))?;

        Ok(
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendTransferTx {
                from_address: payload.sender,
                tx,
            })
            .into(),
        )
    }

    /// Send transfer transaction
    pub async fn send_transfer_transaction(
        from_address: H160,
        tx: DidTransaction,
    ) -> BftResult<Brc20BridgeOpImpl> {
        let withdraw = Withdrawal::get()
            .map_err(|err| Error::FailedToProgress(format!("cannot get withdraw: {err:?}")))?;

        withdraw
            .send_transaction(tx.clone().into())
            .await
            .map_err(|err| Error::FailedToProgress(format!("cannot send transfer tx: {err:?}")))?;

        // Mark the reveal UTXO as used
        let outpoint = tx.0.input[0].previous_output;
        withdraw.mark_reveal_utxo_as_used(&outpoint);

        Ok(
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::TransferTxSent { from_address, tx })
                .into(),
        )
    }
}
