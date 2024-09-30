use std::collections::HashMap;

use candid::Principal;
use serde::{Deserialize, Serialize};

use super::principals::CanisterPrincipals;
use super::CanisterType;
use crate::contracts::EvmNetwork;

/// canister_ids.json db
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CanistersDb {
    #[serde(flatten)]
    canisters: HashMap<CanisterType, CanisterPrincipals>,
}

impl CanistersDb {
    /// Get the principal ID of a canister based on the network type.
    pub fn get(&self, canister: CanisterType, network: EvmNetwork) -> Option<Principal> {
        self.canisters
            .get(&canister)
            .and_then(|canister| canister.get(network))
            .copied()
    }

    /// set a new canister principal to the map.
    ///
    /// If the entry already exists, it will be updated.
    pub fn set(&mut self, canister: CanisterType, principal: Principal, network: EvmNetwork) {
        self.canisters
            .entry(canister)
            .and_modify(|canister_data| canister_data.set(principal, network))
            .or_insert_with(|| CanisterPrincipals::new(principal, network));
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.canisters.len()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_should_read_insert_and_update_data() {
        let mut db = CanistersDb::default();

        db.set(
            CanisterType::Brc20,
            Principal::from_text("v5vof-zqaaa-aaaal-ai5cq-cai").unwrap(),
            EvmNetwork::Localhost,
        );

        // set for mainnet

        db.set(
            CanisterType::Brc20,
            Principal::from_text("v2uir-uiaaa-aaaal-ai5ca-cai").unwrap(),
            EvmNetwork::Mainnet,
        );

        assert_eq!(
            db.get(CanisterType::Brc20, EvmNetwork::Mainnet).unwrap(),
            Principal::from_text("v2uir-uiaaa-aaaal-ai5ca-cai").unwrap()
        );

        // check for localhost

        assert_eq!(
            db.get(CanisterType::Brc20, EvmNetwork::Localhost).unwrap(),
            Principal::from_text("v5vof-zqaaa-aaaal-ai5cq-cai").unwrap()
        );
    }
}
