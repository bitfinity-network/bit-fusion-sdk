use std::path::PathBuf;

use crate::contracts::IcNetwork;

const LOCAL_DIR: &str = ".dfx/local/";
const TESTNET_DIR: &str = ".dfx/testnet/";
const FILENAME: &str = "canister_ids.json";

/// A struct to represent the path of the `canister_ids.json` file.
#[derive(Debug, Clone)]
pub enum CanisterIdsPath {
    Localhost,
    Testnet,
    Mainnet,
    /// Custom path with network type.
    CustomPath(PathBuf, IcNetwork),
}

impl CanisterIdsPath {
    /// Get the path of the `canister_ids.json` file.
    pub fn path(&self) -> PathBuf {
        match self {
            Self::CustomPath(path, _) => path.clone(),
            _ => self.default_path(),
        }
    }

    /// Get the default path of the `canister_ids.json` file, based on network type
    fn default_path(&self) -> PathBuf {
        let mut path = PathBuf::from("./");

        match self {
            Self::Localhost => {
                path.push(LOCAL_DIR);
            }
            Self::Testnet => {
                path.push(TESTNET_DIR);
            }
            _ => {}
        };

        path.push(FILENAME);

        path
    }
}

impl From<IcNetwork> for CanisterIdsPath {
    fn from(network: IcNetwork) -> Self {
        match network {
            IcNetwork::Localhost => Self::Localhost,
            IcNetwork::Testnet => Self::Testnet,
            IcNetwork::Mainnet => Self::Mainnet,
        }
    }
}

impl From<&CanisterIdsPath> for IcNetwork {
    fn from(path: &CanisterIdsPath) -> Self {
        match path {
            CanisterIdsPath::Localhost => IcNetwork::Localhost,
            CanisterIdsPath::Testnet => IcNetwork::Testnet,
            CanisterIdsPath::Mainnet => IcNetwork::Mainnet,
            CanisterIdsPath::CustomPath(_, network) => *network,
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_should_get_path_for_custom_path() {
        let path = PathBuf::from("/tmp/canister_ids.json");
        let canister_ids_path = CanisterIdsPath::CustomPath(path.clone(), IcNetwork::Localhost);

        assert_eq!(canister_ids_path.path(), path);
    }

    #[test]
    fn test_should_get_path_for_localhost() {
        let canister_ids_path = CanisterIdsPath::Localhost;
        let path = canister_ids_path.path();

        let expected = PathBuf::from("./").join(LOCAL_DIR).join(FILENAME);

        assert_eq!(path, expected);
    }

    #[test]
    fn test_should_get_path_for_testnet() {
        let canister_ids_path = CanisterIdsPath::Testnet;
        let path = canister_ids_path.path();

        let expected = PathBuf::from("./").join(TESTNET_DIR).join(FILENAME);

        assert_eq!(path, expected);
    }

    #[test]
    fn test_should_get_path_for_mainnet() {
        let canister_ids_path = CanisterIdsPath::Mainnet;
        let path = canister_ids_path.path();

        let expected = PathBuf::from("./").join(FILENAME);

        assert_eq!(path, expected);
    }
}
