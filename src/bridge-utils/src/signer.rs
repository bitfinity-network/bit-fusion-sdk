use eth_signer::ic_sign::SigningKeyId;
use eth_signer::sign_strategy::{
    ManagementCanisterSigner, SigningStrategy, TransactionSigner, TxSigner,
};
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{CellStructure, StableCell};
use minter_did::error::{Error, Result};

/// A component that provides the access to the signer
pub struct SignerStorage<M: Memory>(StableCell<TxSigner, M>);

impl<M: Memory> SignerStorage<M> {
    /// Stores a new SignerInfo in the given memory.
    pub fn default(memory: M) -> Self {
        let signer =
            TxSigner::ManagementCanister(ManagementCanisterSigner::new(SigningKeyId::Test, vec![]));

        let cell = StableCell::new(memory, signer.clone())
            .expect("failed to initialize transaction signer");

        Self(cell)
    }

    /// Reset the signer with the given strategy and chain id.
    pub fn reset(&mut self, signing_type: SigningStrategy, chain_id: u32) -> Result<()> {
        let signer = signing_type
            .make_signer(chain_id as _)
            .map_err(|e| Error::from(format!("failed to init signer: {e}")))?;

        self.0
            .set(signer)
            .expect("failed to update transaction signer");

        log::trace!("Signer reset finished");

        Ok(())
    }

    /// Returns transaction signer
    pub fn get_transaction_signer(&self) -> impl TransactionSigner {
        self.0.get().clone()
    }
}
