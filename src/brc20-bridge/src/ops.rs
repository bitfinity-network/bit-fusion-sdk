use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use did::H160;
use ic_exports::icrc_types::icrc1::account::Account as IcrcAccount;

use crate::api::{Brc20InscribeError, Brc20InscribeStatus, Erc20MintError, Erc20MintStatus};
use crate::state::State;

pub async fn brc20_to_erc20(
    _state: Rc<RefCell<State>>,
    _eth_address: H160,
) -> Vec<Result<Erc20MintStatus, Erc20MintError>> {
    todo!()
}

pub async fn mint_erc20(
    _state: &RefCell<State>,
    _eth_address: H160,
    _amount: u64,
    _nonce: u32,
) -> Result<Erc20MintStatus, Erc20MintError> {
    todo!()
}

pub(crate) async fn burn_brc20(
    state: &RefCell<State>,
    request_id: u32,
    address: &str,
    amount: u64,
) -> Result<Brc20InscribeStatus, Brc20InscribeError> {
    log::trace!("Transferring {amount} BRC20 to {address} with request id {request_id}");

    state
        .borrow_mut()
        .burn_request_store_mut()
        .insert(request_id, address.to_string(), amount);

    let inscriber = state.borrow().inscriber();
    let fee = state.borrow().inscriber_fee();
    let account = get_brc20_withdrawal_account(inscriber).await?;

    // ICRC1 takes fee on top of the amount
    let to_transfer = amount - fee;
    transfer_brc20(inscriber, account, to_transfer, fee).await?;

    state
        .borrow_mut()
        .burn_request_store_mut()
        .set_transferred(request_id);

    let result = request_btc_withdrawal(inscriber, address.to_string(), to_transfer).await;

    if result.is_ok() {
        state
            .borrow_mut()
            .burn_request_store_mut()
            .remove(request_id);
    }

    result
}

async fn get_brc20_withdrawal_account(
    _inscriber: Principal,
) -> Result<IcrcAccount, Brc20InscribeError> {
    todo!()
}

async fn transfer_brc20(
    _inscriber: Principal,
    _account: IcrcAccount,
    _amount: u64,
    _fee: u64,
) -> Result<(), Brc20InscribeError> {
    todo!()
}

async fn request_btc_withdrawal(
    _inscriber: Principal,
    _address: String,
    _amount: u64,
) -> Result<Brc20InscribeStatus, Brc20InscribeError> {
    todo!()
}
