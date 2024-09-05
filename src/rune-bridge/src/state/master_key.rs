use bitcoin::bip32::ChainCode;
use bitcoin::PublicKey;
use ic_exports::ic_cdk::api::management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId};
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{CellStructure as _, MemoryId, MemoryManager, StableCell, Storable};

use crate::key::KeyError;
use crate::memory::MASTER_KEY_MEMORY_ID;

pub struct MasterKeyStorage<M: Memory> {
    master_key: StableCell<Option<MasterKey>, M>,
}

impl<M> MasterKeyStorage<M>
where
    M: Memory,
{
    /// Creates a new MasterKeyStorage.
    pub fn new(memory: &dyn MemoryManager<M, MemoryId>) -> Self {
        Self {
            master_key: StableCell::new(memory.get(MASTER_KEY_MEMORY_ID), None)
                .expect("stable memory master key initialization failed"),
        }
    }

    /// Returns the master key if it exists.
    pub fn get(&self) -> &Option<MasterKey> {
        self.master_key.get()
    }

    /// Sets the master key.
    pub fn set(&mut self, master_key: MasterKey) {
        self.master_key
            .set(Some(master_key))
            .expect("failed to set master key");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterKey {
    /// Public key encoded in a bytes buffer
    /// - 0 byte for compressed flag (1 if compressed, 0 if uncompressed)
    /// - 1..34 bytes for compressed public key
    /// - 1..66 bytes for uncompressed public key
    public_key: [u8; 66],
    chain_code: [u8; 32],
    pub key_id: EcdsaKeyId,
}

impl MasterKey {
    pub fn new(public_key: PublicKey, chain_code: ChainCode, key_id: EcdsaKeyId) -> Self {
        let mut pubkey_buf = [0; 66];
        pubkey_buf[0] = public_key.compressed as u8;
        let pubkey_bytes = public_key.to_bytes();
        pubkey_buf[1..=pubkey_bytes.len()].copy_from_slice(&pubkey_bytes);

        MasterKey {
            public_key: pubkey_buf,
            chain_code: chain_code.to_bytes(),
            key_id,
        }
    }

    pub fn public_key(&self) -> Result<PublicKey, KeyError> {
        let compressed = self.public_key[0] == 1;
        let last_byte = if compressed { 34 } else { 66 };

        PublicKey::from_slice(&self.public_key[1..last_byte])
            .map_err(|_| KeyError::InvalidPublicKey)
    }

    pub fn chain_code(&self) -> ChainCode {
        ChainCode::from(self.chain_code)
    }
}

impl Storable for MasterKey {
    const BOUND: ic_stable_structures::Bound = ic_stable_structures::Bound::Bounded {
        max_size: 66 + 32 + 1 + 1 + 255,
        is_fixed_size: false,
    };

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let mut buf = Vec::with_capacity(Self::BOUND.max_size() as usize);
        buf.extend_from_slice(&self.public_key);
        buf.extend_from_slice(&self.chain_code);

        let curve_byte = match self.key_id.curve {
            EcdsaCurve::Secp256k1 => 0,
        };
        buf.push(curve_byte);
        // push the length of the name
        buf.push(self.key_id.name.len() as u8);
        buf.extend_from_slice(self.key_id.name.as_bytes());

        buf.into()
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        let mut offset = 0;
        let public_key: [u8; 66] = bytes[offset..offset + 66]
            .try_into()
            .expect("invalid public key");
        offset += 66;
        let chain_code: [u8; 32] = bytes[offset..offset + 32]
            .try_into()
            .expect("invalid chain code");
        offset += 32;
        let curve = match bytes[offset] {
            0 => EcdsaCurve::Secp256k1,
            _ => panic!("unsupported curve"),
        };
        offset += 1;
        let name_len = bytes[offset] as usize;
        offset += 1;
        let name = String::from_utf8(bytes[offset..offset + name_len].to_vec()).unwrap();

        MasterKey {
            public_key,
            chain_code,
            key_id: EcdsaKeyId { curve, name },
        }
    }
}

#[cfg(test)]
mod test {
    use bitcoin::key::Secp256k1;
    use bitcoin::secp256k1::SecretKey;
    use ic_stable_structures::default_ic_memory_manager;

    use super::*;

    #[test]
    fn test_master_key_storage_compressed() {
        let memory_manager = default_ic_memory_manager();
        let mut storage = MasterKeyStorage::new(&memory_manager);

        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let secp_pubkey =
            bitcoin::secp256k1::PublicKey::from_secret_key(&Secp256k1::new(), &secret_key);
        let public_key = PublicKey::new(secp_pubkey);

        let master_key = MasterKey::new(
            public_key,
            ChainCode::from([2; 32]),
            EcdsaKeyId {
                curve: EcdsaCurve::Secp256k1,
                name: "key".to_string(),
            },
        );

        storage.set(master_key.clone());
        assert_eq!(storage.get().as_ref().unwrap(), &master_key);
    }

    #[test]
    fn test_master_key_storage_uncompressed() {
        let memory_manager = default_ic_memory_manager();
        let mut storage = MasterKeyStorage::new(&memory_manager);

        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let secp_pubkey =
            bitcoin::secp256k1::PublicKey::from_secret_key(&Secp256k1::new(), &secret_key);
        let public_key = PublicKey::new_uncompressed(secp_pubkey);

        let master_key = MasterKey::new(
            public_key,
            ChainCode::from([2; 32]),
            EcdsaKeyId {
                curve: EcdsaCurve::Secp256k1,
                name: "key".to_string(),
            },
        );

        storage.set(master_key.clone());
        assert_eq!(storage.get().as_ref().unwrap(), &master_key);
    }
}
