use std::cell::RefCell;
use std::rc::Rc;

use did::H160;

use crate::api::{Brc20InscribeError, Brc20InscribeStatus, Erc20MintError, Erc20MintStatus};
use crate::state::State;

/// Swap a BRC20 for an ERC20.
///
/// This burns a BRC20 and mints an equivalent ERC20.
pub(crate) async fn brc20_to_erc20(
    _state: Rc<RefCell<State>>,
    _eth_address: H160,
) -> Vec<Result<Erc20MintStatus, Erc20MintError>> {
    todo!()
}

/// Swap an ERC20 for a BRC20.
///
/// This burns an ERC20 and inscribes an equivalent BRC20.
pub(crate) async fn erc20_to_brc20(
    _state: &RefCell<State>,
    _request_id: u32,
    _address: &str,
    _amount: u64,
) -> Result<Brc20InscribeStatus, Brc20InscribeError> {
    todo!()
}
