use evm_canister_client::IcCanisterClient;
use ic_exports::candid::{CandidType, Nat, Principal};
use ic_exports::ic_kit::ic;
use icrc_client::account::Account;
use icrc_client::transfer::{TransferArg, TransferError};
use icrc_client::transfer_from::{TransferFromArgs, TransferFromError};
use icrc_client::IcrcCanisterClient;
use minter_did::error::{Error, Result};
use serde::Deserialize;

use super::icrc1::{self, get_token_configuration};

#[derive(Debug, Deserialize, CandidType, Clone)]
pub struct Success {
    pub tx_id: Nat,
    pub amount: Nat,
}

/// Performs mint approval on an ICRC-2 token canister.
///
/// If token fee changed and not equal to cached value,
/// cache will be updated and operation will be retried.
///
/// Returns approved allowance in case of success.
///
/// # Errors
/// - If `amount < fee * 2` returns `Error::InvalidBurnTransaction`, because
/// mint operation requires two transactions: approve and transferFrom.
///
/// - If approval fails, returns `Error::Icrc2ApproveError`.
///
/// - If token canister is not available, returns `Error::InternalError`.
#[async_recursion::async_recursion]
pub async fn mint(
    token: Principal,
    recipient: Principal,
    amount: Nat,
    repeat_on_bad_fee: bool,
) -> Result<Success> {
    let fee = get_token_configuration(token).await?.fee;

    let icrc_client = IcrcCanisterClient::new(IcCanisterClient::new(token));

    if amount < fee {
        return Err(Error::InvalidBurnOperation(format!(
            "{} tokens is not enough to pay transfer fee {}",
            amount, fee
        )));
    }

    // Fee deduction for approve operation.
    let effective_amount = amount.clone() - fee.clone();

    if effective_amount == 0_u64 {
        return Err(Error::InvalidBurnOperation(
            "the effective amount to be transferred is 0".to_string(),
        ));
    }

    let args = TransferArg {
        to: recipient.into(),
        memo: None,
        amount: effective_amount.clone(),
        fee: Some(fee),
        from_subaccount: None,
        created_at_time: None, // Todo: set the time to prevent double spend
    };

    let transfer_result = icrc_client.icrc1_transfer(args).await?;

    if repeat_on_bad_fee {
        if let Err(TransferError::BadFee { .. }) = &transfer_result {
            icrc1::refresh_token_configuration(token).await?;
            return mint(token, recipient, amount, false).await;
        }
    }

    Ok(Success {
        tx_id: transfer_result.map_err(Error::Icrc2TransferError)?,
        amount: effective_amount,
    })
}

/// Performs a transfer from the `from` account to the minter canister main account.
#[async_recursion::async_recursion]
pub async fn burn(
    token: Principal,
    from: Account,
    amount: Nat,
    repeat_on_bad_fee: bool,
) -> Result<Success> {
    let icrc_client = IcrcCanisterClient::new(IcCanisterClient::new(token));

    let minter_canister_account = Account::from(ic::id());

    if amount == 0_u64 {
        return Err(Error::InvalidBurnOperation(
            "the amount to be transferred is 0".to_string(),
        ));
    }

    let args = TransferFromArgs {
        from,
        spender_subaccount: None,
        to: minter_canister_account,
        amount: amount.clone(),
        fee: None,
        memo: None,
        created_at_time: None,
    };

    let transfer_result = icrc_client.icrc2_transfer_from(args).await?;

    if repeat_on_bad_fee {
        if let Err(TransferFromError::BadFee { .. }) = &transfer_result {
            icrc1::refresh_token_configuration(token).await?;
            return burn(token, from, amount, false).await;
        }
    }

    Ok(Success {
        tx_id: transfer_result.map_err(Error::Icrc2TransferFromError)?,
        amount,
    })
}
