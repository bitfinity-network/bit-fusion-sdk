use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use foundry_compilers::artifacts::{EvmVersion, Optimizer, Remapping, Settings, remappings};
use foundry_compilers::solc::SolcCompiler;
use foundry_compilers::{ProjectBuilder, ProjectPathsConfig};

/// The number of parallel Solidity compiler jobs to use when compiling the
/// project.
const SOLC_JOBS: usize = 10;
/// Number of runs to perform when optimizing the contracts.
const RUNS: usize = 2_000;

fn main() -> anyhow::Result<()> {
    const ROOT_DIR: &str = "solidity";
    const REMAPPINGS_FILE: &str = "remappings.txt";

    let root: PathBuf = get_workspace_root_dir()?.join(ROOT_DIR);
    let mut optimizer = Optimizer::default();
    optimizer.enable();
    optimizer.runs(RUNS);

    let settings = Settings {
        optimizer,
        evm_version: Some(EvmVersion::Paris),
        ..Default::default()
    };

    let root = foundry_compilers::utils::canonicalize(root)?;
    let mut paths = ProjectPathsConfig::builder()
        .sources(root.join("src"))
        .artifacts(root.join("out"))
        .build_infos(root.join("out").join("build-info"))
        .lib(root.join("dependencies"))
        .root(root.clone())
        .build()?;

    let remappings_file = root.join(REMAPPINGS_FILE);

    if remappings_file.exists() {
        let remappings = parse_remappings_file(&remappings_file)?;
        paths.remappings.extend(remappings);
    } else {
        anyhow::bail!("remappings file doesn't exist, please check again");
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
