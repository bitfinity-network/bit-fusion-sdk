use bridge_canister::bridge::OperationContext;
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::{BftResult, Error};
use bridge_did::operations::{Brc20BridgeDepositOp, DepositRequest};
use bridge_did::order::{MintOrder, SignedMintOrder};
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;

use super::{Brc20BridgeOp, Brc20BridgeOpImpl};
use crate::core::deposit::Brc20Deposit;

pub struct Brc20BridgeDepositOpImpl;

impl Brc20BridgeDepositOpImpl {
    /// Await for deposit inputs
    pub async fn await_inputs(
        state: RuntimeState<Brc20BridgeOpImpl>,
        request: DepositRequest,
    ) -> BftResult<Brc20BridgeOpImpl> {
        let deposit = Brc20Deposit::get(state.clone())
            .map_err(|err| Error::FailedToProgress(format!("cannot deposit: {err:?}")))?;
        let utxos = deposit
            .get_inputs(&request.dst_address)
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!("cannot find deposit inputs: {err:?}"))
            })?;

        if utxos.is_empty() {
            return Err(Error::FailedToProgress("no inputs".to_string()));
        }

        Ok(
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::AwaitConfirmations {
                deposit: request,
                utxos,
            })
            .into(),
        )
    }

    /// Await for minimum IC confirmations
    pub async fn await_confirmations(
        state: RuntimeState<Brc20BridgeOpImpl>,
        deposit_request: DepositRequest,
        utxos: Vec<Utxo>,
        nonce: u32,
    ) -> BftResult<Brc20BridgeOpImpl> {
        let DepositRequest {
            amount,
            brc20_tick,
            dst_address,
            dst_token,
        } = deposit_request;

        let deposit = Brc20Deposit::get(state.clone())
            .map_err(|err| Error::FailedToProgress(format!("cannot deposit: {err:?}")))?;
        deposit
            .check_confirmations(&dst_address, &utxos)
            .await
            .map_err(|err| Error::FailedToProgress(format!("inputs are not confirmed: {err:?}")))?;

        // check balance
        let brc20_balance = deposit
            .get_brc20_balance(&dst_address, &brc20_tick)
            .await
            .map_err(|err| Error::FailedToProgress(format!("cannot get brc20 balance: {err:?}")))?;

        let brc20_info =
            deposit
                .get_brc20_info(&brc20_tick)
                .await
                .ok_or(Error::FailedToProgress(format!(
                    "cannot get brc20 info for {brc20_tick}"
                )))?;

        if amount > brc20_balance {
            return Err(Error::FailedToProgress(format!(
                "requested amount {amount} is bigger than actual balance {brc20_balance}"
            )));
        }

        let unsigned_mint_order =
            deposit.create_unsigned_mint_order(&dst_address, &dst_token, amount, brc20_info, nonce);

        // mark utxos as used
        deposit
            .mark_utxos_as_used(&dst_address, &utxos)
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!("cannot mark utxos as used: {err:?}"))
            })?;

        Ok(Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::SignMintOrder(unsigned_mint_order)).into())
    }

    /// Sign the provided mint order
    pub async fn sign_mint_order(
        ctx: RuntimeState<Brc20BridgeOpImpl>,
        nonce: u32,
        mut mint_order: MintOrder,
    ) -> BftResult<Brc20BridgeOpImpl> {
        // update nonce
        mint_order.nonce = nonce;

        let deposit = Brc20Deposit::get(ctx)
            .map_err(|err| Error::FailedToProgress(format!("cannot deposit: {err:?}")))?;
        let signed = deposit
            .sign_mint_order(mint_order)
            .await
            .map_err(|err| Error::FailedToProgress(format!("cannot sign mint order: {err:?}")))?;

        Ok(Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::SendMintOrder(signed)).into())
    }

    /// Send the signed mint order to the bridge
    pub async fn send_mint_order(
        ctx: RuntimeState<Brc20BridgeOpImpl>,
        order: SignedMintOrder,
    ) -> BftResult<Brc20BridgeOpImpl> {
        let tx_id = ctx.send_mint_transaction(&order).await?;

        Ok(
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::ConfirmMintOrder {
                signed_mint_order: order,
                tx_id,
            })
            .into(),
        )
    }
}
