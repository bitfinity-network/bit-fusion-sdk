use candid::Principal;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::contracts::IcNetwork;

/// A struct to represent the principal IDs of a canister.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct CanisterPrincipals {
    #[serde(skip_serializing_if = "Option::is_none")]
    ic: Option<Principal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    local: Option<Principal>,
}

impl CanisterPrincipals {
    /// Creates a new `CanisterPrincipals` instance.
    pub fn new(principal: Principal, network: IcNetwork) -> Self {
        let mut instance = Self::default();
        instance.set(principal, network);

        instance
    }

    /// Set the principal ID of a canister based on the network type.
    pub fn set(&mut self, principal: Principal, network: IcNetwork) {
        debug!("Setting canister principal {principal} for network: {network:?}");
        match network {
            IcNetwork::Localhost => self.local = Some(principal),
            IcNetwork::Ic => self.ic = Some(principal),
        }
    }

    /// Get the principal ID of a canister based on the network type.
    pub fn get(&self, network: IcNetwork) -> Option<&Principal> {
        match network {
            IcNetwork::Localhost => self.local.as_ref(),
            IcNetwork::Ic => self.ic.as_ref(),
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
        let canister_principal = CanisterPrincipals::new(principal, IcNetwork::Localhost);

        assert_eq!(canister_principal.local, Some(principal));
        assert_eq!(canister_principal.ic, None);

        let principal = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        let canister_principal = CanisterPrincipals::new(principal, IcNetwork::Ic);
        assert_eq!(canister_principal.local, None);
        assert_eq!(canister_principal.ic, Some(principal));
    }

    #[test]
    fn test_should_set() {
        let principal = Principal::from_text("rwlgt-iiaaa-aaaaa-aaaaa-cai").unwrap();
        let mut canister_principal = CanisterPrincipals::default();
        canister_principal.set(principal, IcNetwork::Localhost);

        assert_eq!(canister_principal.local, Some(principal));
    }
}
