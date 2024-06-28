use std::collections::HashMap;
use std::fs::{create_dir_all, read_to_string};
use std::path::{Path, PathBuf};
use std::process::Command;

use error::{SolidityHelperError, SolidityHelperResult};
use serde::{Deserialize, Serialize};

const BUILD_INFO_DIR: &str = "build-info";
const CONTRACTS_SUBPATHS: &[&str] = &["src/"];

pub mod error;

pub struct SolidityContract {
    pub name: String,
    pub bytecode: Vec<u8>,
    pub bytecode_hex: String,
    pub deployed_bytecode: Vec<u8>,
    pub deployed_bytecode_hex: String,
    pub method_identifiers: HashMap<String, String>,
}

/// Solidity contracts build outputs
pub struct BuiltSolidityContracts {
    pub contracts: HashMap<String, SolidityContract>,
    pub output_dir: PathBuf,
}

/// Solidity file times with output directory
#[derive(Debug, Serialize, Deserialize)]
pub struct SolidityFileTimes {
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

/// Solidity contracts builder
pub struct SolidityBuilder {
    /// The subpaths to watch for changes
    subpaths_to_watch: Vec<PathBuf>,
    /// The path to the file that stores the last modified time of the files
    times_file_path: PathBuf,
    /// The solidity root path; where the contracts are
    solidity_root_path: PathBuf,
    /// workspace root path
    workspace_root_path: PathBuf,
}

impl SolidityBuilder {
    /// Tries to create a new SolidityBuilder
    pub fn new() -> SolidityHelperResult<Self> {
        let workspace_root_path = Self::workspace_root_path()?;
        let solidity_root_path = workspace_root_path.join("solidity");

        let subpaths_to_watch = CONTRACTS_SUBPATHS
            .iter()
            .map(|subpath| solidity_root_path.join(subpath))
            .collect::<Vec<_>>();

        let times_file_path = workspace_root_path.join("target/.solidity_file_timestamps.json");

        Ok(Self {
            subpaths_to_watch,
            times_file_path,
            solidity_root_path,
            workspace_root_path,
        })
    }

    /// Builds the updated contracts
    pub fn build_updated_contracts(&self) -> SolidityHelperResult<BuiltSolidityContracts> {
        let current_file_times = self.load_file_times().ok();

        let solidity_output_dir = current_file_times
            .as_ref()
            .map(|current_file_times| current_file_times.output_dir.clone())
            .unwrap_or_else(|| {
                self.workspace_root_path
                    .join("target/")
                    .join(format!("{}", rand::random::<u64>()))
            });

        let contracts = if current_file_times
            .as_ref()
            .map(|current_file_times| {
                self.have_files_changed(current_file_times, &solidity_output_dir)
            })
            .unwrap_or(Ok(true))?
        {
            let contracts = self.compile_solidity_contracts(&solidity_output_dir)?;
            // update times
            self.save_file_times(&solidity_output_dir)?;

            contracts
        } else {
            self.get_solidity_contracts(&solidity_output_dir)?
        };

        Ok(BuiltSolidityContracts {
            contracts,
            output_dir: solidity_output_dir,
        })
    }

    /// Load file times from the file in the times_file_path (under target/)
    pub fn load_file_times(&self) -> SolidityHelperResult<SolidityFileTimes> {
        let reader = std::fs::File::open(&self.times_file_path)?;

        Ok(serde_json::from_reader(reader)?)
    }

    /// Checks whether the files have changed since the last time they were checked
    pub fn have_files_changed(
        &self,
        current_file_times: &SolidityFileTimes,
        output_dir: &Path,
    ) -> SolidityHelperResult<bool> {
        let new_file_times = self.scan_file_times(output_dir)?;
        // first check whether there are new files
        if current_file_times.files.len() != new_file_times.files.len() {
            return Ok(true);
        }

        // check whether the files have changed
        Ok(current_file_times
            .files
            .keys()
            .chain(new_file_times.files.keys())
            .any(|key| current_file_times.files.get(key) != new_file_times.files.get(key)))
    }

    /// Saves the file times to the times file
    fn save_file_times(&self, output_dir: &Path) -> SolidityHelperResult<()> {
        let file_times = self.scan_file_times(output_dir)?;
        let file = std::fs::File::create(&self.times_file_path)?;
        serde_json::to_writer(file, &file_times)?;

        Ok(())
    }

    /// Scan the file times in the subpaths and return the SolidityFileTimes structure
    fn scan_file_times(&self, output_dir: &Path) -> SolidityHelperResult<SolidityFileTimes> {
        let mut file_times = SolidityFileTimes::new(output_dir.to_path_buf());

        for subpath in &self.subpaths_to_watch {
            Self::scan_dir_file_times(&mut file_times, subpath)?;
        }

        Ok(file_times)
    }

    /// Recursive function to scan the file times in a directory
    fn scan_dir_file_times(files: &mut SolidityFileTimes, path: &Path) -> SolidityHelperResult<()> {
        for file_in_dir in std::fs::read_dir(path)? {
            let file_in_dir = file_in_dir?;
            let file_path = file_in_dir.path();

            if file_path.is_dir() {
                Self::scan_dir_file_times(files, &file_path)?;
            } else {
                let metadata = file_in_dir.metadata()?;
                let modified_time = metadata
                    .modified()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();

                files.files.insert(file_path, modified_time.as_secs());
            }
        }

        Ok(())
    }

    /// Compiles all smart contracts in a path using forge
    fn compile_solidity_contracts(
        &self,
        output_dir: &Path,
    ) -> SolidityHelperResult<HashMap<String, SolidityContract>> {
        create_dir_all(output_dir)?;

        let output_path_str = output_dir.to_str().ok_or_else(|| {
            SolidityHelperError::GenericError(format!(
                "cannot convert path [{}] to string",
                output_dir.display()
            ))
        })?;

        Command::new("forge")
            .args(["build", "--out", output_path_str, "--skip", "test"])
            .current_dir(self.solidity_root_path.as_path())
            .output()?;

        self.get_solidity_contracts(output_dir)
    }

    fn get_solidity_contracts(
        &self,
        output_dir: &Path,
    ) -> SolidityHelperResult<HashMap<String, SolidityContract>> {
        let contract_paths = self.contract_paths(output_dir)?;

        let mut contracts = HashMap::new();

        for (name, path) in contract_paths {
            // This is a hack to ignore the build-info folder since it's not a
            // contract
            if path
                .to_str()
                .expect("should be possible to convert path to string")
                .contains(BUILD_INFO_DIR)
            {
                continue;
            }

            println!(
                "Parsing compiled contract [{name}] from path: [{}]",
                path.display()
            );
            let json_contract = read_to_string(&path)?;
            let solidity_contract = Self::parse_json_contract(&json_contract, &name)?;
            contracts.insert(name, solidity_contract);
        }

        Ok(contracts)
    }

    /// Returns a map with:
    /// key: contract name
    /// value: compiled JSON contract path
    fn contract_paths(&self, output_dir: &Path) -> SolidityHelperResult<HashMap<String, PathBuf>> {
        let json_only = |p: &PathBuf| p.extension().map(|e| e == "json").unwrap_or(false);
        let stem_and_path = |p: PathBuf| {
            // Strip the file extension
            p.file_stem()
                // convert it to a string
                .and_then(|stem| stem.to_str().map(ToString::to_string))
                // ... and return a tuple of the file name without extension,
                // and the path it self
                .map(|s| (s, p))
        };
        let paths = output_dir
            .read_dir()? // all directories in the root
            .flatten()
            .flat_map(|dir| dir.path().read_dir()) // read all sub directories
            .flatten()
            .flatten()
            // ignore errors...
            .map(|e| e.path())
            .filter(json_only) // filter out anything that isn't a .json file
            .filter_map(stem_and_path)
            .collect::<HashMap<_, _>>();

        Ok(paths)
    }

    /// Parses a JSON compiled contract file
    fn parse_json_contract(
        json_contract: &str,
        contract_name: &str,
    ) -> SolidityHelperResult<SolidityContract> {
        let contract_value = serde_json::from_str::<serde_json::Value>(json_contract)?;

        let bytecode_hex = contract_value
            .get("bytecode")
            .and_then(|v| v.get("object"))
            .and_then(|v| v.as_str())
            .map(|v| v.trim_start_matches("0x").to_string())
            .ok_or(SolidityHelperError::JsonFieldNotFoundError("bytecode"))?;
        let bytecode = hex::decode(&bytecode_hex)?;

        let deployed_bytecode_hex = contract_value
            .get("deployedBytecode")
            .and_then(|v| v.get("object"))
            .and_then(|v| v.as_str())
            .map(|v| v.trim_start_matches("0x").to_string())
            .ok_or(SolidityHelperError::JsonFieldNotFoundError(
                "deployedBytecode",
            ))?;
        let deployed_bytecode = hex::decode(&deployed_bytecode_hex)?;

        let method_identifiers = contract_value.get("methodIdentifiers").cloned(); // Some contracts compiled are missing methodIdentifiers

        let method_identifiers = if let Some(method_identifiers) = method_identifiers {
            serde_json::from_value(method_identifiers)?
        } else {
            HashMap::new()
        };

        Ok(SolidityContract {
            bytecode,
            bytecode_hex,
            deployed_bytecode,
            deployed_bytecode_hex,
            name: contract_name.to_owned(),
            method_identifiers,
        })
    }

    #[inline]
    /// Returns the workspace root path
    fn workspace_root_path() -> SolidityHelperResult<PathBuf> {
        let workspace_root_path: PathBuf = std::env::var("CARGO_MANIFEST_DIR")
            .map_err(|err| {
                SolidityHelperError::GenericError(format!(
                    "cannot read CARGO_MANIFEST_DIR env var. Error: {err:?}"
                ))
            })?
            .into();

        // Tricky hack, there's no clean way in cargo to get the workspace root folder
        let workspace_root_path: PathBuf = workspace_root_path
            .parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| {
                SolidityHelperError::GenericError(
                    "cannot access workspace parent folder.".to_string(),
                )
            })?
            .into();

        Ok(workspace_root_path)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn should_return_the_contract_paths() {
        let builder = SolidityBuilder::new().unwrap();
        let built_contracts = builder.build_updated_contracts().unwrap();
        let path = &built_contracts.output_dir;
        assert!(path.exists());
        assert!(path.is_dir());

        let paths = builder.contract_paths(path).unwrap();
        let precompiles_path = paths.get("TestContractWithPrecompiles").unwrap();
        assert!(precompiles_path.exists());
        assert!(precompiles_path.is_file());
    }

    #[test]
    fn should_build_the_custom_token_smart_contract() {
        let builder = SolidityBuilder::new().unwrap();
        let contracts = builder.build_updated_contracts().unwrap().contracts;

        let contract = contracts.get("TestContractWithPrecompiles").unwrap();
        assert!(!contract.bytecode.is_empty());
        assert!(!contract.deployed_bytecode.is_empty());
        assert!(contract.method_identifiers.contains_key("do_ripemd160()"));
    }

    #[test]
    fn test_should_detect_changes_when_building_contracts() {
        let times_file_path = tempfile::NamedTempFile::new().unwrap();
        let workpace_tmp_root = tempfile::tempdir().unwrap();
        let solidity_tmp_root = workpace_tmp_root.path().join("solidity");
        let solidity_tmp_subpath = solidity_tmp_root.join("src");
        // create
        std::fs::create_dir_all(&solidity_tmp_subpath).unwrap();
        // write solidity contract
        write_solidity_contract(&solidity_tmp_subpath, "TestContract");

        let mut builder = SolidityBuilder::new().unwrap();
        builder.workspace_root_path = workpace_tmp_root.path().to_path_buf();
        builder.solidity_root_path.clone_from(&solidity_tmp_root);
        builder.times_file_path = times_file_path.path().to_path_buf();
        builder.subpaths_to_watch = vec![solidity_tmp_subpath.clone()];

        // build
        let contracts = builder.build_updated_contracts().unwrap().contracts;
        assert!(contracts.get("TestContract").is_some());

        // get current times
        let current_times = builder.load_file_times().unwrap();
        // should not have changed
        assert!(!builder
            .have_files_changed(&current_times, &solidity_tmp_root)
            .unwrap());

        // update file
        std::thread::sleep(Duration::from_secs(1));
        write_solidity_contract(&solidity_tmp_subpath, "TestContract");
        // should have changed
        assert!(builder
            .have_files_changed(&current_times, &solidity_tmp_root)
            .unwrap());

        // should change if new file
        builder.save_file_times(&solidity_tmp_root).unwrap();
        let current_times = builder.load_file_times().unwrap();
        // should not have changed
        assert!(!builder
            .have_files_changed(&current_times, &solidity_tmp_root)
            .unwrap());
        write_solidity_contract(&solidity_tmp_subpath, "TestContractV2");
        // build
        let contracts = builder.build_updated_contracts().unwrap().contracts;
        assert!(contracts.get("TestContract").is_some());
        // should have changed
        assert!(builder
            .have_files_changed(&current_times, &solidity_tmp_root)
            .unwrap());

        // remove all
        workpace_tmp_root.close().unwrap();
    }

    fn write_solidity_contract(p: &Path, name: &str) -> PathBuf {
        let p = p.join(format!("{name}.sol"));

        let contract = format!(
            r#"
        pragma solidity ^0.8.0;

        contract {name} {{
            function doSomething() public pure returns (uint256) {{
                return 42;
            }}
        }}
        "#
        );

        std::fs::write(&p, contract).unwrap();

        p
    }
}
