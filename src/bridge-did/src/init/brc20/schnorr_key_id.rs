use candid::CandidType;
use serde::Deserialize;

use crate::schnorr::{SchnorrAlgorithm, SchnorrKeyId};

/// Schnorr key IDs
#[derive(Debug, Clone, PartialEq, Eq, CandidType, Deserialize)]
pub enum SchnorrKeyIds {
    #[allow(unused)]
    TestKeyLocalDevelopment,
    #[allow(unused)]
    TestKey1,
    #[allow(unused)]
    ProductionKey1,
}

impl SchnorrKeyIds {
    /// Converts the key ID to a Schnorr key ID
    pub fn to_key_id(&self, algorithm: SchnorrAlgorithm) -> SchnorrKeyId {
        SchnorrKeyId {
            algorithm,
            name: match self {
                Self::TestKeyLocalDevelopment => "dfx_test_key",
                Self::TestKey1 => "test_key_1",
                Self::ProductionKey1 => "key_1",
            }
            .to_string(),
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_should_convert_to_key_id() {
        let key_id = SchnorrKeyIds::TestKeyLocalDevelopment.to_key_id(SchnorrAlgorithm::Ed25519);
        assert_eq!(key_id.algorithm, SchnorrAlgorithm::Ed25519);
        assert_eq!(key_id.name, "dfx_test_key");

        let key_id = SchnorrKeyIds::TestKey1.to_key_id(SchnorrAlgorithm::Ed25519);
        assert_eq!(key_id.algorithm, SchnorrAlgorithm::Ed25519);
        assert_eq!(key_id.name, "test_key_1");

        let key_id = SchnorrKeyIds::ProductionKey1.to_key_id(SchnorrAlgorithm::Ed25519);
        assert_eq!(key_id.algorithm, SchnorrAlgorithm::Ed25519);
        assert_eq!(key_id.name, "key_1");
    }
}
