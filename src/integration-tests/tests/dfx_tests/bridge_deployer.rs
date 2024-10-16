mod cli_args;
mod eval;

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::context::CanisterType;

use super::DfxTestContext;

async fn setup() -> DfxTestContext {
    DfxTestContext::new(&CanisterType::EVM_TEST_SET).await
}

#[tokio::test]
#[cfg(feature = "dfx_tests")]
async fn test_should_deploy_rune_bridge() {
    use cli_args::{CommonCliArgs, DeployCliArgs};
    use tempfile::TempDir;

    use crate::context::TestContext;

    let ctx = setup().await;

    let CommonCliArgs {
        evm: evm_principal,
        private_key,
        identity_path,
    } = CommonCliArgs::new(&ctx).await;

    let DeployCliArgs {
        wasm_path,
        wallet_canister,
    } = DeployCliArgs::new(&ctx, CanisterType::RuneBridge).await;

    let admin_principal = ctx.admin().to_text();

    println!("IDENTITY_PATH: {}", identity_path.display());
    println!("PRIVATE_KEY: {}", private_key);
    println!("ADMIN_PRINCIPAL: {}", admin_principal);
    println!("WALLET_CANISTER: {}", wallet_canister);
    println!("EVM_PRINCIPAL: {}", evm_principal);
    println!("WASM_PATH: {}", wasm_path.display());

    // get the output dir for evaluated trycmd files
    let trycmd_output_dir = TempDir::new().expect("failed to create temp file");

    // eval the trycmd files
    eval::eval_trycmd(
        [
            ("IDENTITY_PATH", identity_path.display().to_string()),
            ("PRIVATE_KEY", private_key.to_string()),
            ("ADMIN_PRINCIPAL", admin_principal.to_string()),
            ("WALLET_CANISTER", wallet_canister.to_string()),
            ("EVM_PRINCIPAL", evm_principal.to_string()),
            ("WASM_PATH", wasm_path.display().to_string()),
        ],
        Path::new("./tests/bridge_deployer/deploy"),
        trycmd_output_dir.path(),
        "rune_bridge.trycmd",
    )
    .expect("failed to eval trycmd files");

    // change cwd to workspace root
    let workspace_root = get_workspace_root().expect("failed to get workspace root");
    std::env::set_current_dir(&workspace_root).expect("failed to change cwd");
    println!(
        "Working directory: {}",
        std::env::current_dir().unwrap().display()
    );

    let case = format!("{}/*.eval.trycmd", trycmd_output_dir.path().display());
    println!("Running tests at {case}...");

    trycmd::TestCases::new().case(&case).run();

    trycmd_output_dir.close().expect("failed to close temp dir");
}

#[tokio::test]
#[cfg(feature = "dfx_tests")]
async fn test_should_deploy_erc20_bridge() {
    use cli_args::{CommonCliArgs, DeployCliArgs};
    use tempfile::TempDir;

    use crate::context::TestContext;

    let ctx = setup().await;

    let CommonCliArgs {
        evm: evm_principal,
        private_key,
        identity_path,
    } = CommonCliArgs::new(&ctx).await;

    let DeployCliArgs {
        wasm_path,
        wallet_canister,
    } = DeployCliArgs::new(&ctx, CanisterType::Erc20Bridge).await;

    let admin_principal = ctx.admin().to_text();

    println!("IDENTITY_PATH: {}", identity_path.display());
    println!("PRIVATE_KEY: {}", private_key);
    println!("ADMIN_PRINCIPAL: {}", admin_principal);
    println!("WALLET_CANISTER: {}", wallet_canister);
    println!("EVM_PRINCIPAL: {}", evm_principal);
    println!("WASM_PATH: {}", wasm_path.display());

    // get the output dir for evaluated trycmd files
    let trycmd_output_dir = TempDir::new().expect("failed to create temp file");

    // eval the trycmd files
    eval::eval_trycmd(
        [
            ("IDENTITY_PATH", identity_path.display().to_string()),
            ("PRIVATE_KEY", private_key.to_string()),
            ("ADMIN_PRINCIPAL", admin_principal.to_string()),
            ("WALLET_CANISTER", wallet_canister.to_string()),
            ("EVM_PRINCIPAL", evm_principal.to_string()),
            ("WASM_PATH", wasm_path.display().to_string()),
        ],
        Path::new("./tests/bridge_deployer/deploy"),
        trycmd_output_dir.path(),
        "erc20_bridge.trycmd",
    )
    .expect("failed to eval trycmd files");

    // change cwd to workspace root
    let workspace_root = get_workspace_root().expect("failed to get workspace root");
    std::env::set_current_dir(&workspace_root).expect("failed to change cwd");
    println!(
        "Working directory: {}",
        std::env::current_dir().unwrap().display()
    );

    let case = format!("{}/*.eval.trycmd", trycmd_output_dir.path().display());
    println!("Running tests at {case}...");

    trycmd::TestCases::new().case(&case).run();

    trycmd_output_dir.close().expect("failed to close temp dir");
}

fn get_workspace_root() -> anyhow::Result<PathBuf> {
    let cmd_output = Command::new("cargo")
        .args(["metadata", "--format-version=1"])
        .output()?;

    if !cmd_output.status.success() {
        anyhow::bail!("Failed to get workspace root");
    }

    let json =
        serde_json::from_str::<serde_json::Value>(String::from_utf8(cmd_output.stdout)?.as_str())?;

    let path = json
        .get("workspace_root")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get workspace root"))?;

    Ok(PathBuf::from(path))
}
