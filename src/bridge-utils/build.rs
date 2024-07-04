use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use foundry_compilers::artifacts::{remappings, EvmVersion, Optimizer, Remapping, Settings};
use foundry_compilers::solc::SolcCompiler;
use foundry_compilers::{ProjectBuilder, ProjectPathsConfig};

/// The number of parallel Solidity compiler jobs to use when compiling the
/// project.
/// This value is set to 10 to take advantage of modern multi-core CPUs and speed up the compilation process.
const SOLC_JOBS: usize = 10;

fn main() -> anyhow::Result<()> {
    const ROOT_DIR: &str = "solidity";
    const REMAPPINGS_FILE: &str = "remappings.txt";

    let root: PathBuf = get_workspace_root_dir()?.join(ROOT_DIR);
    let mut optimizer = Optimizer::default();
    optimizer.enable();
    optimizer.runs(20_000);

    let settings = Settings {
        optimizer,
        evm_version: Some(EvmVersion::Paris),
        ..Default::default()
    };
    let mut paths = ProjectPathsConfig::dapptools(&root)?;

    let remappings_file = root.join(REMAPPINGS_FILE);

    if remappings_file.exists() {
        let remappings = parse_remappings_file(&remappings_file)?;
        paths.remappings.extend(remappings);
    }

    let project = ProjectBuilder::<SolcCompiler>::new(Default::default())
        .solc_jobs(SOLC_JOBS)
        .paths(paths)
        .ephemeral()
        .settings(settings)
        .ignore_paths(vec![root.join("test")])
        .build(SolcCompiler::AutoDetect)?;

    let output = project.compile()?;

    // Tell Cargo that if a source file changes, to rerun this build script.
    project.rerun_if_sources_changed();

    assert!(!output.has_compiler_errors(), "{}", output.to_string());
    Ok(())
}

pub fn get_workspace_root_dir() -> anyhow::Result<PathBuf> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .ok_or(anyhow::anyhow!("Could not find workspace root"))?
        .to_path_buf())
}

/// Parses a remappings file at the given path and returns a vector of
/// remappings.
fn parse_remappings_file(path: &Path) -> anyhow::Result<Vec<remappings::Remapping>> {
    // Read the file contents
    let contents = fs::read_to_string(path)?;

    // Split the lines
    let lines = contents.lines();
    let mut remappings = Vec::new();
    for line in lines {
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let remapping = Remapping::from_str(line)?;
        remappings.push(remapping);
    }

    Ok(remappings)
}
