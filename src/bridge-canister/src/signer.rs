use std::cell::RefCell;

use bridge_did::error::{Error, Result};
use eth_signer::ic_sign::SigningKeyId;
use eth_signer::sign_strategy::{
    ManagementCanisterSigner, SigningStrategy, TransactionSigner, TxSigner,
};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableCell, VirtualMemory};

use crate::memory::{MEMORY_MANAGER, TX_SIGNER_MEMORY_ID};

/// A component that provides the access to the signer
#[derive(Default, Clone)]
pub struct SignerStorage {}

impl SignerStorage {
    /// Reset the signer with the given strategy and chain id.
    pub fn reset(&self, strategy: SigningStrategy, chain_id: u32) -> Result<()> {
        let signer = strategy
            .clone()
            .make_signer(chain_id as _)
            .map_err(|e| Error::from(format!("failed to init signer: {e}")))?;

        TX_SIGNER.with(|s| {
            s.borrow_mut()
                .set(signer)
                .expect("failed to update transaction signer")
        });

        log::trace!("Set up signer with signing strategy: {strategy:?}");

        Ok(())
    }

    /// Returns transaction signer
    pub fn get_transaction_signer(&self) -> impl TransactionSigner {
        TX_SIGNER.with(|s| s.borrow().get().clone())
    }
}

thread_local! {
    static TX_SIGNER: RefCell<StableCell<TxSigner, VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(
            StableCell::new(MEMORY_MANAGER.with(|mm| mm.get(TX_SIGNER_MEMORY_ID)),
            TxSigner::ManagementCanister(ManagementCanisterSigner::new(SigningKeyId::Test,
            vec![],
        ))).expect("Failed to initialize transaction signer"))
}
