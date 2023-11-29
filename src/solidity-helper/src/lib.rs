use std::collections::HashMap;
use std::fs::{create_dir_all, read_to_string};
use std::path::PathBuf;
use std::process::Command;

use error::SolidityHelperError;

pub mod error;
pub struct SolidityContract {
    pub name: String,
    pub bytecode: Vec<u8>,
    pub bytecode_hex: String,
    pub deployed_bytecode: Vec<u8>,
    pub deployed_bytecode_hex: String,
    pub method_identifiers: HashMap<String, String>,
}

/// Compiles all smart contracts in a path using forge
pub fn compile_solidity_contracts(
    contracts_subpath: Option<&str>,
    output_path: Option<&str>,
) -> Result<HashMap<String, SolidityContract>, SolidityHelperError> {
    let output_path = compile(
        contracts_subpath.map(Into::into),
        output_path.map(Into::into),
    )?;
    let contract_paths = contract_paths(output_path)?;

    let mut contracts = HashMap::new();

    for (name, path) in contract_paths {
        println!(
            "Parsing compiled contract [{name}] from path: [{}]",
            path.display()
        );
        let json_contract = read_to_string(&path)?;
        let solidity_contract = parse_json_contract(&json_contract, &name)?;
        contracts.insert(name, solidity_contract);
    }

    Ok(contracts)
}

/// Compiles the solidity contracts in the folder. Returns the output path
fn compile(
    contracts_subpath: Option<PathBuf>,
    output_path: Option<PathBuf>,
) -> Result<PathBuf, SolidityHelperError> {
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
            SolidityHelperError::GenericError("cannot access workspace parent folder.".to_string())
        })?
        .into();

    let solidity_root_path = workspace_root_path.join("solidity");
    let solidity_root_path = contracts_subpath
        .map(|contracts_subpath| solidity_root_path.join(contracts_subpath))
        .unwrap_or(solidity_root_path);

    let output_path = output_path.map(PathBuf::from).unwrap_or_else(|| {
        let target_path = workspace_root_path.join("target");
        target_path.join(format!("{}", rand::random::<u64>()))
    });

    create_dir_all(&output_path)?;

    let output_path_str = output_path.to_str().ok_or_else(|| {
        SolidityHelperError::GenericError(format!(
            "cannot convert path [{output_path:?}] to string"
        ))
    })?;

    Command::new("forge")
        .args(["build", "--out", output_path_str, "--skip", "test"])
        .current_dir(solidity_root_path)
        .output()?;

    Ok(output_path)
}

/// Returns a map with:
/// key: contract name
/// value: compiled JSON contract path
fn contract_paths(root: PathBuf) -> Result<HashMap<String, PathBuf>, SolidityHelperError> {
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
    let paths = root
        .read_dir()? // all directories in the root
        .flatten()
        .flat_map(|dir| dir.path().read_dir()) // read all sub directories
        .flatten()
        .flatten() // ignore errors...
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
) -> Result<SolidityContract, SolidityHelperError> {
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

    let method_identifiers = contract_value.get("methodIdentifiers").ok_or(
        SolidityHelperError::JsonFieldNotFoundError("methodIdentifiers"),
    )?;
    let method_identifiers: HashMap<String, String> =
        serde_json::from_value(method_identifiers.clone())?;

    Ok(SolidityContract {
        bytecode,
        bytecode_hex,
        deployed_bytecode,
        deployed_bytecode_hex,
        name: contract_name.to_owned(),
        method_identifiers,
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn should_return_the_contract_paths() {
        let path = compile(None, None).unwrap();
        assert!(path.exists());
        assert!(path.is_dir());

        let paths = contract_paths(path).unwrap();
        let precompiles_path = paths.get("TestContractWithPrecompiles").unwrap();
        assert!(precompiles_path.exists());
        assert!(precompiles_path.is_file());
    }

    #[test]
    fn should_build_the_custom_token_smart_contract() {
        let contracts = compile_solidity_contracts(None, None).unwrap();
        let contract = contracts.get("TestContractWithPrecompiles").unwrap();
        assert!(!contract.bytecode.is_empty());
        assert!(!contract.deployed_bytecode.is_empty());
        assert!(contract.method_identifiers.get("do_ripemd160()").is_some());
    }
}
