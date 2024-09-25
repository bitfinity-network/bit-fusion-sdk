use candid::Principal;
use serde::{Deserialize, Serialize};

use crate::contracts::EvmNetwork;

/// A struct to represent the principal IDs of a canister.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct CanisterPrincipal {
    #[serde(skip_serializing_if = "Option::is_none")]
    ic: Option<Principal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    local: Option<Principal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    testnet: Option<Principal>,
}

impl CanisterPrincipal {
    /// Creates a new `CanisterPrincipal` instance.
    pub fn new(principal: Principal, network: EvmNetwork) -> Self {
        let mut instance = Self::default();
        instance.set(principal, network);

        instance
    }

    /// Set the principal ID of a canister based on the network type.
    pub fn set(&mut self, principal: Principal, network: EvmNetwork) {
        match network {
            EvmNetwork::Localhost => self.local = Some(principal),
            EvmNetwork::Testnet => self.testnet = Some(principal),
            EvmNetwork::Mainnet => self.ic = Some(principal),
        }
    }

    /// Get the principal ID of a canister based on the network type.
    pub fn get(&self, network: EvmNetwork) -> Option<&Principal> {
        match network {
            EvmNetwork::Localhost => self.local.as_ref(),
            EvmNetwork::Testnet => self.testnet.as_ref(),
            EvmNetwork::Mainnet => self.ic.as_ref(),
        }
    }
}

#[cfg(test)]
mod test {
    use candid::Principal;

    use super::*;

    #[test]
    fn test_canister_principal() {
        let principal = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        let canister_principal = CanisterPrincipal::new(principal, EvmNetwork::Localhost);

        assert_eq!(canister_principal.local, Some(principal));
        assert_eq!(canister_principal.testnet, None);
        assert_eq!(canister_principal.ic, None);

        let principal = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        let canister_principal = CanisterPrincipal::new(principal, EvmNetwork::Testnet);
        assert_eq!(canister_principal.local, None);
        assert_eq!(canister_principal.testnet, Some(principal));
        assert_eq!(canister_principal.ic, None);

        let principal = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        let canister_principal = CanisterPrincipal::new(principal, EvmNetwork::Mainnet);
        assert_eq!(canister_principal.local, None);
        assert_eq!(canister_principal.testnet, None);
        assert_eq!(canister_principal.ic, Some(principal));
    }

    #[test]
    fn test_should_set() {
        let principal = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        let mut canister_principal = CanisterPrincipal::default();
        canister_principal.set(principal, EvmNetwork::Localhost);

        let other_principal = Principal::from_text("v5vof-zqaaa-aaaal-ai5cq-cai").unwrap();
        canister_principal.set(other_principal, EvmNetwork::Testnet);

        assert_eq!(canister_principal.local, Some(principal));
        assert_eq!(canister_principal.testnet, Some(other_principal));
        assert_eq!(canister_principal.ic, None);
    }
}
