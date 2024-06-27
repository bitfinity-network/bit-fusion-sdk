use std::collections::HashMap;
use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use solidity_helper::error::SolidityHelperError;
use solidity_helper::{compile_solidity_contracts, get_solidity_contracts, SolidityContract};

const CONTRACTS_SUBPATHS: &[&str] = &[
    "src/",
    "lib/forge-std/src/",
    "lib/openzeppelin-contracts/contracts/",
    "lib/openzeppelin-contracts-upgradeable/contracts/",
    "lib/openzeppelin-foundry-upgrades/src/",
];

fn main() -> anyhow::Result<()> {
    let file_times_builder = SolidityFileTimesWatcher::new(CONTRACTS_SUBPATHS);
    let current_file_times = file_times_builder.load_file_times().ok();

    let contracts = if file_times_builder.has_files_changed(current_file_times.as_ref()) {
        let output_path = file_times_builder.solidity_output_dir.clone();
        let contracts = match compile_solidity_contracts(None, Some(
            output_path.display().to_string().as_str(),
        )) {
            Ok(c) => c,
            Err(SolidityHelperError::IoError(err)) if err.kind() == ErrorKind::NotFound => {
                return Err(anyhow::anyhow!(
                    "`forge` executable not found. Try installing forge with foundry: https://book.getfoundry.sh/getting-started/installation or check if it is present in the PATH"
                ))
            }
            Err(err) => {
                return Err(anyhow::anyhow!("Failed to compile solidity contracts: {err:?}"))
            }
        };

        // update times
        file_times_builder
            .save_file_times()
            .expect("failed to save file times");

        contracts
    } else {
        get_solidity_contracts(current_file_times.unwrap().output_dir)?
    };

    set_contract_code(
        &contracts,
        "WrappedToken",
        "BUILD_SMART_CONTRACT_WRAPPED_TOKEN_HEX_CODE",
    );
    set_contract_code(
        &contracts,
        "BFTBridge",
        "BUILD_SMART_CONTRACT_BFT_BRIDGE_HEX_CODE",
    );
    set_contract_code(
        &contracts,
        "FeeCharge",
        "BUILD_SMART_CONTRACT_FEE_CHARGE_HEX_CODE",
    );
    set_deployed_contract_code(
        &contracts,
        "BFTBridge",
        "BUILD_SMART_CONTRACT_BFT_BRIDGE_DEPLOYED_HEX_CODE",
    );
    set_contract_code(
        &contracts,
        "WatermelonToken",
        "BUILD_SMART_CONTRACT_TEST_WTM_HEX_CODE",
    );

    set_contract_code(
        &contracts,
        "UUPSProxy",
        "BUILD_SMART_CONTRACT_UUPS_PROXY_HEX_CODE",
    );

    Ok(())
}

/// Loads the contract with the specified name
fn set_contract_code(
    contracts: &HashMap<String, SolidityContract>,
    contract_name: &str,
    env_var: &str,
) {
    let contract_hex = &get_solidity_contract(contracts, contract_name).bytecode_hex;

    set_var(env_var, contract_hex);
}

/// Loads the deployed contract bytecode with the specified name
fn set_deployed_contract_code(
    contracts: &HashMap<String, SolidityContract>,
    contract_name: &str,
    env_var: &str,
) {
    let deployed_contract_hex =
        &get_solidity_contract(contracts, contract_name).deployed_bytecode_hex;

    set_var(env_var, deployed_contract_hex);
}

fn get_solidity_contract<'a>(
    contracts: &'a HashMap<String, SolidityContract>,
    contract_name: &str,
) -> &'a SolidityContract {
    contracts
        .get(contract_name)
        .unwrap_or_else(|| panic!("Cannot find the {contract_name} contract"))
}

// this sets a compile time variable
fn set_var(key: &str, value: &str) {
    println!("cargo:rustc-env={key}={value}");
}

struct SolidityFileTimesWatcher {
    subpaths: Vec<PathBuf>,
    times_file_path: PathBuf,
    solidity_output_dir: PathBuf,
}

impl SolidityFileTimesWatcher {
    /// Creates a new SolidityFileTimesWatcher
    pub fn new(subpaths: &'static [&'static str]) -> Self {
        let root_path =
            solidity_helper::workspace_root_path().expect("Failed to get workspace root path");

        let solidity_root_path = solidity_helper::solidity_root_path(&root_path, None)
            .expect("Failed to get solidity root path");

        let subpaths = subpaths
            .iter()
            .map(|subpath| solidity_root_path.join(subpath))
            .collect::<Vec<_>>();

        let times_file_path = root_path.join("target/.solidity_file_timestamps.toml");

        let solidity_output_dir = root_path
            .join("target/")
            .join(format!("{}", rand::random::<u64>()));

        Self {
            subpaths,
            times_file_path,
            solidity_output_dir,
        }
    }

    /// Checks whether the files have changed since the last time they were checked
    pub fn has_files_changed(&self, current_file_times: Option<&SolidityFileTimes>) -> bool {
        let current_file_times = match current_file_times {
            Some(file_times) => file_times,
            None => {
                println!("Current file times is None. Assuming files have changed");
                return true;
            }
        };
        let new_file_times = self.scan_file_times().expect("Failed to scan file times");
        // first check whether there are new files
        if current_file_times.files.len() != new_file_times.files.len() {
            return true;
        }

        // check whether the files have changed
        current_file_times
            .files
            .keys()
            .chain(new_file_times.files.keys())
            .any(|key| current_file_times.files.get(key) != new_file_times.files.get(key))
    }

    /// Saves the file times to the times file
    pub fn save_file_times(&self) -> anyhow::Result<()> {
        let file_times = self.scan_file_times()?;
        let file_times = toml::to_string(&file_times)?;

        let mut file = std::fs::File::create(&self.times_file_path)?;
        file.write_all(file_times.as_bytes())?;

        Ok(())
    }

    fn load_file_times(&self) -> anyhow::Result<SolidityFileTimes> {
        let file_times = std::fs::read_to_string(&self.times_file_path)?;

        Ok(toml::from_str(&file_times)?)
    }

    fn scan_file_times(&self) -> anyhow::Result<SolidityFileTimes> {
        let mut file_times = SolidityFileTimes::new(self.solidity_output_dir.clone());

        for subpath in &self.subpaths {
            Self::scan_dir_file_times(&mut file_times, subpath)?;
        }

        Ok(file_times)
    }

    fn scan_dir_file_times(files: &mut SolidityFileTimes, path: &Path) -> anyhow::Result<()> {
        for file_in_dir in std::fs::read_dir(path)? {
            let file_in_dir = file_in_dir?;
            let file_path = file_in_dir.path();

            if file_path.is_dir() {
                Self::scan_dir_file_times(files, &file_path)?;
            } else {
                let metadata = file_in_dir.metadata()?;
                let modified_time = metadata.modified()?.duration_since(std::time::UNIX_EPOCH)?;

                files.files.insert(file_path, modified_time.as_secs());
            }
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SolidityFileTimes {
    /// Map between the file path to modified time
    files: HashMap<PathBuf, u64>,
    output_dir: PathBuf,
}

impl SolidityFileTimes {
    pub fn new(output_dir: PathBuf) -> Self {
        Self {
            files: HashMap::new(),
            output_dir,
        }
    }
}
