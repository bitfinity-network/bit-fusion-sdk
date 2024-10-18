use std::path::{Path, PathBuf};

use once_cell::sync::OnceCell;
use tokio::io::AsyncReadExt;

use crate::utils::get_workspace_root_dir;

const BTC_CANISTER_MOCK_WASM_FILENAME: &str = "ic-bitcoin-canister-mock.wasm.gz";
const IC_CKBTC_MINTER_WASM_FILENAME: &str = "ic-ckbtc-minter.wasm.gz";
const IC_CKBTC_KYT_WASM_FILENAME: &str = "ic-ckbtc-kyt.wasm.gz";
const EVM_RPC_WASM_FILENAME: &str = "evm_rpc.wasm.gz";
const BTC_BRIDGE_WASM_FILENAME: &str = "btc-bridge.wasm.gz";
const ERC20_BRIDGE_WASM_FILENAME: &str = "erc20-bridge.wasm.gz";
const BTC_CANISTER_WASM_FILENAME: &str = "ic-bitcoin-canister-mock.wasm.gz";
const SIGNATURE_VERIFICATION_WASM_FILENAME: &str = "signature_verification.wasm.gz";
const EVM_TESTNET_WASM_FILENAME: &str = "evm_testnet.wasm.gz";
const ICRC1_LEDGER_WASM_FILENAME: &str = "icrc1-ledger.wasm.gz";
const ICRC2_BRIDGE_WASM_FILENAME: &str = "icrc2-bridge.wasm.gz";
const RUNE_BRIDGE_WASM_FILENAME: &str = "rune-bridge.wasm.gz";
const BRC20_BRIDGE_WASM_FILENAME: &str = "brc20-bridge.wasm.gz";

pub async fn get_icrc1_token_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, ICRC1_LEDGER_WASM_FILENAME).await
}

pub async fn get_icrc1_token_canister_wasm_path() -> PathBuf {
    get_wasm_path(ICRC1_LEDGER_WASM_FILENAME).await
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
    get_or_load_wasm(&CANISTER_BYTECODE, SIGNATURE_VERIFICATION_WASM_FILENAME).await
}

/// Returns the path to the signature_verification canister
pub async fn get_signature_verification_canister_wasm_path() -> PathBuf {
    get_wasm_path(SIGNATURE_VERIFICATION_WASM_FILENAME).await
}

/// Returns the bytecode of the minter evm
pub async fn get_ck_erc20_bridge_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, ERC20_BRIDGE_WASM_FILENAME).await
}

/// Returns the path to the erc20 bridge canister
pub async fn get_ck_erc20_bridge_canister_wasm_path() -> PathBuf {
    get_wasm_path(ERC20_BRIDGE_WASM_FILENAME).await
}

/// Returns the bytecode of the evmc canister - Testnet
pub async fn get_evm_testnet_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, EVM_TESTNET_WASM_FILENAME).await
}

/// Returns the path to the evm testnet canister
pub async fn get_evm_testnet_canister_wasm_path() -> PathBuf {
    get_wasm_path(EVM_TESTNET_WASM_FILENAME).await
}

/// Returns the bytecode of the evmc canister - Testnet
pub async fn get_evm_rpc_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, EVM_RPC_WASM_FILENAME).await
}

/// Returns the path to the evm rpc canister
pub async fn get_evm_rpc_canister_wasm_path() -> PathBuf {
    get_wasm_path(EVM_RPC_WASM_FILENAME).await
}

/// Returns the bytecode of the minter canister
pub async fn get_icrc2_bridge_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, ICRC2_BRIDGE_WASM_FILENAME).await
}

/// Returns the path to the icrc2 bridge canister
pub async fn get_icrc2_bridge_canister_wasm_path() -> PathBuf {
    get_wasm_path(ICRC2_BRIDGE_WASM_FILENAME).await
}

pub async fn get_btc_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, BTC_CANISTER_MOCK_WASM_FILENAME).await
}

/// Returns the path to the btc canister
pub async fn get_btc_canister_wasm_path() -> PathBuf {
    get_wasm_path(BTC_CANISTER_WASM_FILENAME).await
}

/// Returns the bytecode of the minter canister
pub async fn get_ck_btc_minter_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, IC_CKBTC_MINTER_WASM_FILENAME).await
}

/// Returns the path to the minter canister
pub async fn get_ck_btc_minter_canister_wasm_path() -> PathBuf {
    get_wasm_path(IC_CKBTC_MINTER_WASM_FILENAME).await
}

/// Returns the bytecode of the kyt canister
pub async fn get_kyt_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, IC_CKBTC_KYT_WASM_FILENAME).await
}

/// Returns the path to the kyt canister
pub async fn get_kyt_canister_wasm_path() -> PathBuf {
    get_wasm_path(IC_CKBTC_KYT_WASM_FILENAME).await
}

/// Returns the bytecode of the evm bridge canister
pub async fn get_btc_bridge_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, BTC_BRIDGE_WASM_FILENAME).await
}

/// Returns the path to the btc bridge canister
pub async fn get_btc_bridge_canister_wasm_path() -> PathBuf {
    get_wasm_path(BTC_BRIDGE_WASM_FILENAME).await
}

/// Returns the bytecode of the rune bridge canister
pub async fn get_rune_bridge_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, RUNE_BRIDGE_WASM_FILENAME).await
}

/// Returns the path to the rune bridge canister
pub async fn get_rune_bridge_canister_wasm_path() -> PathBuf {
    get_wasm_path(RUNE_BRIDGE_WASM_FILENAME).await
}

/// Returns the bytecode of the brc20 bridge canister
pub async fn get_brc20_bridge_canister_bytecode() -> Vec<u8> {
    static CANISTER_BYTECODE: OnceCell<Vec<u8>> = OnceCell::new();
    get_or_load_wasm(&CANISTER_BYTECODE, BRC20_BRIDGE_WASM_FILENAME).await
}

/// Returns the path to the brc20 bridge canister
pub async fn get_brc20_bridge_canister_wasm_path() -> PathBuf {
    get_wasm_path(BRC20_BRIDGE_WASM_FILENAME).await
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

pub async fn get_wasm_path(file_name: &str) -> PathBuf {
    get_path_to_file(file_name).await
}

async fn check_file_exists(path: &Path) -> bool {
    tokio::fs::metadata(path).await.is_ok()
}
