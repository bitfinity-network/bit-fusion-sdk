use std::cell::RefCell;

use bitcoin::bip32::{ChildNumber, DerivationPath, Xpub};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Address;
use did::H160;

use crate::interface::GetAddressError;
use crate::state::State;

pub const DERIVATION_PATH_PREFIX: u8 = 7;

pub fn get_deposit_address(
    state: &RefCell<State>,
    eth_address: &H160,
) -> Result<Address, GetAddressError> {
    let state = state.borrow();
    let public_key = state.public_key();
    let chain_code = state.chain_code();
    let x_public_key = Xpub {
        network: state.network(),
        depth: 0,
        parent_fingerprint: Default::default(),
        child_number: ChildNumber::from_normal_idx(0).map_err(|_| GetAddressError::Derivation)?,
        public_key: public_key.inner,
        chain_code,
    };
    let derivation_path =
        get_derivation_path(eth_address).map_err(|_| GetAddressError::Derivation)?;
    let public_key = x_public_key
        .derive_pub(&Secp256k1::new(), &derivation_path)
        .map_err(|_| GetAddressError::Derivation)?
        .public_key;

    Ok(Address::p2wpkh(&public_key.into(), state.network())
        .expect("used uncompressed public key to derive address"))
}

pub fn get_derivation_path_ic(eth_address: &H160) -> Vec<Vec<u8>> {
    let mut bytes = vec![DERIVATION_PATH_PREFIX];
    bytes.append(&mut eth_address.0 .0.to_vec());

    let mut dp = vec![];
    for slice in bytes.chunks_exact(3) {
        let mut part = vec![0];
        part.append(&mut slice.to_vec());
        dp.push(part);
    }

    dp
}

fn get_derivation_path(eth_address: &H160) -> Result<DerivationPath, GetAddressError> {
    let mut parts = vec![];
    for part in get_derivation_path_ic(eth_address).iter() {
        let child_idx = u32::from_be_bytes(part[..].try_into().unwrap());
        let child = ChildNumber::from_normal_idx(child_idx).unwrap();
        parts.push(child);
    }

    Ok(DerivationPath::from(parts))
}
