//! A module to perform I/O operations on the `canister_ids.json` file.

mod canisters;
mod db;
mod path;
mod principals;

use candid::Principal;
use db::CanistersDb;
use tracing::debug;

pub use self::canisters::CanisterType;
pub use self::path::CanisterIdsPath;

/// A struct to map canister names to their principal IDs, which are serialized into file `canister_ids.json`.
#[derive(Debug, Clone)]
pub struct CanisterIds {
    canisters: CanistersDb,
    path: CanisterIdsPath,
}

impl CanisterIds {
    /// Creates a new `CanisterIds` instance.
    pub fn new(canister_ids_path: CanisterIdsPath) -> Self {
        Self {
            canisters: CanistersDb::default(),
            path: canister_ids_path,
        }
    }

    /// Read the `canister_ids.json` file.
    pub fn read(canister_ids_path: CanisterIdsPath) -> anyhow::Result<Self> {
        let path = canister_ids_path.path();
        debug!("Reading canister IDs from file: {}", path.display());

        // load from json and set the network
        let content = std::fs::read_to_string(&path)?;
        let canisters: CanistersDb = serde_json::from_str(&content)?;

        let mut canisters_ids = Self::new(canister_ids_path);
        canisters_ids.canisters = canisters;

        Ok(canisters_ids)
    }

    /// Read the `canister_ids.json` file or return a default new instance.
    pub fn read_or_default(canister_ids_path: CanisterIdsPath) -> Self {
        Self::read(canister_ids_path.clone()).unwrap_or_else(|_| Self::new(canister_ids_path))
    }

    /// Write the `canister_ids.json` file.
    pub fn write(&self) -> anyhow::Result<()> {
        let path = self.path.path();
        debug!("Writing canister IDs to file: {}", path.display());

        let content = serde_json::to_string_pretty(&self.canisters)?;
        std::fs::write(&path, content)?;

        Ok(())
    }

    /// Get the principal ID of a canister based on the network type.
    pub fn get(&self, canister: CanisterType) -> Option<Principal> {
        self.canisters.get(canister, (&self.path).into())
    }

    /// set a new canister principal to the map.
    ///
    /// If the entry already exists, it will be updated.
    pub fn set(&mut self, canister: CanisterType, principal: Principal) {
        self.canisters.set(canister, principal, (&self.path).into());
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::contracts::EvmNetwork;

    #[test]
    fn test_should_deserialize_canister_ids() {
        let content = r#"
{
  "brc20-bridge": {
    "ic": "v5vof-zqaaa-aaaal-ai5cq-cai"
  },
  "btc-bridge": {
    "ic": "v2uir-uiaaa-aaaal-ai5ca-cai"
  },
  "erc20-bridge": {
    "ic": "vtxdn-caaaa-aaaal-ai5dq-cai"
  }
}
        "#;

        let tempfile = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tempfile.path(), content).unwrap();

        // deserialize
        let canister_ids = CanisterIds::read(CanisterIdsPath::CustomPath(
            tempfile.path().to_path_buf(),
            EvmNetwork::Mainnet,
        ))
        .unwrap();

        assert_eq!(canister_ids.canisters.len(), 3);
        assert_eq!(
            canister_ids.get(CanisterType::Brc20),
            Some(Principal::from_text("v5vof-zqaaa-aaaal-ai5cq-cai").unwrap())
        );
        assert_eq!(
            canister_ids.get(CanisterType::Btc),
            Some(Principal::from_text("v2uir-uiaaa-aaaal-ai5ca-cai").unwrap())
        );
        assert_eq!(
            canister_ids.get(CanisterType::Erc20),
            Some(Principal::from_text("vtxdn-caaaa-aaaal-ai5dq-cai").unwrap())
        );
        assert!(canister_ids.get(CanisterType::Icrc2).is_none());
    }

    #[test]
    fn test_should_add_bridges_and_serialize() {
        let tempfile = tempfile::NamedTempFile::new().unwrap();
        let mut canister_ids = CanisterIds::read_or_default(CanisterIdsPath::CustomPath(
            tempfile.path().to_path_buf(),
            EvmNetwork::Localhost,
        ));

        canister_ids.set(
            CanisterType::Brc20,
            Principal::from_text("v5vof-zqaaa-aaaal-ai5cq-cai").unwrap(),
        );

        canister_ids.set(
            CanisterType::Btc,
            Principal::from_text("v2uir-uiaaa-aaaal-ai5ca-cai").unwrap(),
        );

        canister_ids.write().unwrap();

        // load and check
        let canister_ids = CanisterIds::read(CanisterIdsPath::CustomPath(
            tempfile.path().to_path_buf(),
            EvmNetwork::Localhost,
        ))
        .unwrap();

        assert_eq!(canister_ids.canisters.len(), 2);
        assert_eq!(
            canister_ids.get(CanisterType::Brc20),
            Some(Principal::from_text("v5vof-zqaaa-aaaal-ai5cq-cai").unwrap())
        );
        assert_eq!(
            canister_ids.get(CanisterType::Btc),
            Some(Principal::from_text("v2uir-uiaaa-aaaal-ai5ca-cai").unwrap())
        );
    }

    #[test]
    fn test_should_insert_or_update_data() {
        let mut canister_ids = CanisterIds::new(CanisterIdsPath::Localhost);

        canister_ids.set(
            CanisterType::Brc20,
            Principal::from_text("v5vof-zqaaa-aaaal-ai5cq-cai").unwrap(),
        );

        // set for mainnet
        canister_ids.path = CanisterIdsPath::Mainnet;

        canister_ids.set(
            CanisterType::Brc20,
            Principal::from_text("v2uir-uiaaa-aaaal-ai5ca-cai").unwrap(),
        );

        assert_eq!(
            canister_ids.get(CanisterType::Brc20).unwrap(),
            Principal::from_text("v2uir-uiaaa-aaaal-ai5ca-cai").unwrap()
        );

        // check for localhost
        canister_ids.path = CanisterIdsPath::Localhost;

        assert_eq!(
            canister_ids.get(CanisterType::Brc20).unwrap(),
            Principal::from_text("v5vof-zqaaa-aaaal-ai5cq-cai").unwrap()
        );
    }
}
