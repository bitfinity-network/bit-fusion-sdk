use did::H160;
use ethers_core::utils;
use evm_canister_client::IcCanisterClient;
use ic_exports::candid::{CandidType, Nat, Principal};
use ic_exports::ic_kit::ic;
use ic_exports::icrc_types::icrc1::account::{Account, Subaccount};
use ic_exports::icrc_types::icrc2::approve::{ApproveArgs, ApproveError};
use ic_exports::icrc_types::icrc2::transfer_from::{TransferFromArgs, TransferFromError};
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
pub async fn approve_mint(
    token: Principal,
    spender: Account,
    amount: Nat,
    repeat_on_bad_fee: bool,
) -> Result<Success> {
    let fee = get_token_configuration(token).await?.fee;
    let full_fee = Nat::from(2_u64) * fee.clone();

    let icrc_client = IcrcCanisterClient::new(IcCanisterClient::new(token));

    // Fee deducted twice because there are two transactions: approve and transferFrom.
    if amount < full_fee {
        return Err(Error::InvalidBurnOperation(format!(
            "{} tokens is not enough to pay double fee {}",
            amount, full_fee
        )));
    }

    // Fee deduction for approve operation.
    let effective_amount = amount.clone() - fee.clone();

    let args = ApproveArgs {
        from_subaccount: None,
        spender,
        amount: effective_amount.clone(),
        expected_allowance: Some(0_u64.into()),
        expires_at: Some(u64::MAX),
        fee: Some(fee),
        memo: None,
        created_at_time: None,
    };

    let approve_result = icrc_client.icrc2_approve(args).await?;

    if repeat_on_bad_fee {
        if let Err(ApproveError::BadFee { .. }) = &approve_result {
            icrc1::refresh_token_configuration(token).await?;
            return approve_mint(token, spender, amount, false).await;
        }
    }

    Ok(Success {
        tx_id: approve_result.map_err(Error::Icrc2ApproveError)?,
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

/// Generates a subaccount for which transferFrom will be called.
pub fn approve_subaccount(
    user: H160,
    operation_id: u32,
    chain_id: u32,
    to_token: Principal,
    recipient: Principal,
) -> Subaccount {
    let mut bytes = Vec::with_capacity(128);
    bytes.extend_from_slice(user.0.as_bytes());
    bytes.extend_from_slice(&operation_id.to_be_bytes());
    bytes.extend_from_slice(&chain_id.to_be_bytes());
    bytes.extend_from_slice(to_token.as_slice());
    bytes.extend_from_slice(recipient.as_slice());

    utils::keccak256(bytes)
}
