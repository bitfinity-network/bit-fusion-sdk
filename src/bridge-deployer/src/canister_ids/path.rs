use std::path::PathBuf;

use crate::contracts::IcNetwork;

const LOCAL_DIR: &str = ".dfx/local/";
const FILENAME: &str = "canister_ids.json";

/// A struct to represent the path of the `canister_ids.json` file.
#[derive(Debug, Clone)]
pub enum CanisterIdsPath {
    Localhost,
    Ic,
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

        if let Self::Localhost = self {
            path.push(LOCAL_DIR);
        };

        path.push(FILENAME);

        path
    }
}

impl From<IcNetwork> for CanisterIdsPath {
    fn from(network: IcNetwork) -> Self {
        match network {
            IcNetwork::Localhost => Self::Localhost,
            IcNetwork::Ic => Self::Ic,
        }
    }
}

impl From<&CanisterIdsPath> for IcNetwork {
    fn from(path: &CanisterIdsPath) -> Self {
        match path {
            CanisterIdsPath::Localhost => IcNetwork::Localhost,
            CanisterIdsPath::Ic => IcNetwork::Ic,
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
    fn test_should_get_path_for_mainnet() {
        let canister_ids_path = CanisterIdsPath::Ic;
        let path = canister_ids_path.path();

        let expected = PathBuf::from("./").join(FILENAME);

        assert_eq!(path, expected);
    }
}
