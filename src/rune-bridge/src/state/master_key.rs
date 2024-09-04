use bitcoin::bip32::ChainCode;
use bitcoin::PublicKey;
use ic_exports::ic_cdk::api::management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId};
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{CellStructure as _, MemoryId, MemoryManager, StableCell, Storable};

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
    pub public_key: PublicKey,
    pub chain_code: ChainCode,
    pub key_id: EcdsaKeyId,
}

impl Storable for MasterKey {
    const BOUND: ic_stable_structures::Bound = ic_stable_structures::Bound::Bounded {
        max_size: 65 + 32 + 1 + 1 + 255,
        is_fixed_size: false,
    };

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let mut buf = Vec::with_capacity(Self::BOUND.max_size() as usize);
        buf.extend_from_slice(&self.public_key.to_bytes());
        buf.extend_from_slice(&self.chain_code.to_bytes());

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
        let public_key = PublicKey::from_slice(&bytes[offset..offset + 65]).unwrap();
        offset += 65;
        let chain_code: [u8; 32] = bytes[offset..offset + 32].try_into().unwrap();
        let chain_code = ChainCode::from(chain_code);
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
    use bitcoin::PrivateKey;
    use ic_stable_structures::default_ic_memory_manager;

    use super::*;

    #[test]
    fn test_master_key_storage() {
        let memory_manager = default_ic_memory_manager();
        let mut storage = MasterKeyStorage::new(&memory_manager);

        let private_key = PrivateKey::generate(bitcoin::Network::Bitcoin);
        let public_key = private_key.public_key(&bitcoin::secp256k1::Secp256k1::new());

        let master_key = MasterKey {
            public_key,
            chain_code: ChainCode::from([2; 32]),
            key_id: EcdsaKeyId {
                curve: EcdsaCurve::Secp256k1,
                name: "key".to_string(),
            },
        };

        storage.set(master_key.clone());
        assert_eq!(storage.get().as_ref().unwrap(), &master_key);
    }
}
