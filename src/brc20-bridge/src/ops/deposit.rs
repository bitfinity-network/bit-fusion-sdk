use bridge_canister::bridge::OperationContext;
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::{BftResult, Error};
use bridge_did::order::{MintOrder, SignedMintOrder};
use bridge_utils::bft_events::MintedEventData;
use candid::{CandidType, Deserialize};
use did::{H160, H256};
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use serde::Serialize;

use super::Brc20BridgeOp;
use crate::brc20_info::Brc20Tick;
use crate::core::deposit::Brc20Deposit;

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub struct DepositRequest {
    pub amount: u128,
    pub brc20_tick: Brc20Tick,
    pub dst_address: H160,
    pub dst_token: H160,
}

/// BRC20 bridge deposit operations
#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum Brc20BridgeDepositOp {
    /// Await for deposit inputs
    AwaitInputs(DepositRequest),
    /// Await for minimum IC confirmations
    AwaitConfirmations {
        deposit: DepositRequest,
        utxos: Vec<Utxo>,
    },
    /// Sign the provided mint order
    SignMintOrder(MintOrder),
    /// Send the signed mint order to the bridge
    SendMintOrder(SignedMintOrder),
    /// Confirm the mint order
    ConfirmMintOrder {
        signed_mint_order: SignedMintOrder,
        tx_id: H256,
    },
    /// Mint order confirmed status
    MintOrderConfirmed { data: MintedEventData },
}

impl Brc20BridgeDepositOp {
    /// Await for deposit inputs
    pub async fn await_inputs(
        state: RuntimeState<Brc20BridgeOp>,
        request: DepositRequest,
    ) -> BftResult<Brc20BridgeOp> {
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

        Ok(Brc20BridgeOp::Deposit(Self::AwaitConfirmations {
            deposit: request,
            utxos,
        }))
    }

    /// Await for minimum IC confirmations
    pub async fn await_confirmations(
        state: RuntimeState<Brc20BridgeOp>,
        deposit_request: DepositRequest,
        utxos: Vec<Utxo>,
        nonce: u32,
    ) -> BftResult<Brc20BridgeOp> {
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

        Ok(Brc20BridgeOp::Deposit(Self::SignMintOrder(
            unsigned_mint_order,
        )))
    }

    /// Sign the provided mint order
    pub async fn sign_mint_order(
        ctx: RuntimeState<Brc20BridgeOp>,
        nonce: u32,
        mut mint_order: MintOrder,
    ) -> BftResult<Brc20BridgeOp> {
        // update nonce
        mint_order.nonce = nonce;

        let deposit = Brc20Deposit::get(ctx)
            .map_err(|err| Error::FailedToProgress(format!("cannot deposit: {err:?}")))?;
        let signed = deposit
            .sign_mint_order(mint_order)
            .await
            .map_err(|err| Error::FailedToProgress(format!("cannot sign mint order: {err:?}")))?;

        Ok(Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::SendMintOrder(
            signed,
        )))
    }

    /// Send the signed mint order to the bridge
    pub async fn send_mint_order(
        ctx: RuntimeState<Brc20BridgeOp>,
        order: SignedMintOrder,
    ) -> BftResult<Brc20BridgeOp> {
        let tx_id = ctx.send_mint_transaction(&order).await?;

        Ok(Brc20BridgeOp::Deposit(Self::ConfirmMintOrder {
            signed_mint_order: order,
            tx_id,
        }))
    }
}
