use std::path::{Path, PathBuf};

use once_cell::sync::OnceCell;
use tokio::io::AsyncReadExt;

use crate::utils::get_workspace_root_dir;

pub async fn get_icrc1_token_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, "icrc1-ledger.wasm.gz").await
}

async fn get_or_load_wasm(cell: &OnceCell<Vec<u8>>, file_name: &str) -> Vec<u8> {
    match cell.get() {
        Some(code) => code.clone(),
        None => {
            let code = load_wasm_bytecode_or_panic(file_name).await;
            _ = cell.set(code.clone());
            code
        }
    }
}

/// Returns the bytecode of the signature_verification canister
pub async fn get_signature_verification_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, "signature_verification.wasm.gz").await
}

/// Returns the bytecode of the minter evm
pub async fn get_ck_erc20_minter_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, "erc20-minter.wasm.gz").await
}

/// Returns the bytecode of the evmc canister - Testnet
pub async fn get_evm_testnet_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, "evm_testnet.wasm.gz").await
}

/// Returns the bytecode of the evmc canister - Testnet
pub async fn get_evm_rpc_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, "evm_rpc.wasm.gz").await
}

/// Returns the bytecode of the minter canister
pub async fn get_icrc2_minter_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, "icrc2-minter.wasm.gz").await
}

pub async fn get_btc_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, "ic-bitcoin-canister-mock.wasm.gz").await
}

pub async fn get_ck_btc_minter_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, "ic-ckbtc-minter.wasm.gz").await
}

pub async fn get_kyt_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, "ic-ckbtc-kyt.wasm.gz").await
}

pub async fn get_btc_bridge_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, "btc-bridge.wasm.gz").await
}

pub async fn get_rune_bridge_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, "rune-bridge.wasm.gz").await
}

async fn load_wasm_bytecode_or_panic(wasm_name: &str) -> Vec<u8> {
    let path = get_path_to_file(wasm_name).await;

    let mut f = tokio::fs::File::open(path)
        .await
        .expect("File does not exists");

    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)
        .await
        .expect("Could not read file content");

    buffer
}

pub async fn get_path_to_file(file_name: &str) -> PathBuf {
    if let Ok(dir_path) = std::env::var("WASM_DIR") {
        let file_path = Path::new(&dir_path).join(file_name);
        if check_file_exists(&file_path).await {
            return file_path;
        }
    } else {
        const ARTIFACT_PATH: &str = ".artifact";
        // Get to the root of the project
        let root_dir = get_workspace_root_dir();
        let file_path = root_dir.join(ARTIFACT_PATH).join(file_name);
        if check_file_exists(&file_path).await {
            return file_path;
        }
    }

    if let Ok(dir_path) = std::env::var("DFX_WASMS_DIR") {
        let file_path = Path::new(&dir_path).join(file_name);
        if check_file_exists(&file_path).await {
            return file_path;
        }
    }

    panic!(
        "File {file_name} was not found in dirs provided by ENV variables WASM_DIR or DFX_WASMS_DIR or in the '.artifact' folder"
    );
}

async fn check_file_exists(path: &Path) -> bool {
    tokio::fs::metadata(path).await.is_ok()
}
