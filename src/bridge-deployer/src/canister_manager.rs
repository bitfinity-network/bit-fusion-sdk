use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use crate::commands::Bridge;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DeploymentMode {
    Install,
    Upgrade,
    Reinstall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeploymentInfo {
    timestamp: DateTime<Utc>,
    mode: DeploymentMode,
    configuration: Bridge,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CanisterInfo {
    pub canister_id: String,
    pub canister_type: Bridge,
    pub current_wasm_hash: String,
    pub deployment_history: Vec<DeploymentInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CanisterManager {
    canisters: HashMap<String, CanisterInfo>,
}

impl Default for CanisterManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CanisterManager {
    pub fn new() -> Self {
        Self {
            canisters: HashMap::new(),
        }
    }

    pub fn add_or_update_canister(
        &mut self,
        canister_id: String,
        canister_type: Bridge,
        wasm_hash: String,
        mode: DeploymentMode,
        configuration: Bridge,
    ) {
        let deployment_info = DeploymentInfo {
            timestamp: Utc::now(),
            mode,
            configuration,
        };

        if let Some(info) = self.canisters.get_mut(&canister_id) {
            info.current_wasm_hash = wasm_hash;
            info.deployment_history.push(deployment_info);
        } else {
            let info = CanisterInfo {
                canister_id: canister_id.clone(),
                canister_type,
                current_wasm_hash: wasm_hash,
                deployment_history: vec![deployment_info],
            };
            self.canisters.insert(canister_id, info);
        }
    }

    pub fn get_canister(&self, canister_id: &str) -> Option<&CanisterInfo> {
        self.canisters.get(canister_id)
    }

    pub fn list_canisters(&self) -> Vec<&CanisterInfo> {
        self.canisters.values().collect()
    }

    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let canisters: HashMap<String, CanisterInfo> = serde_json::from_str(&contents)?;
        Ok(Self { canisters })
    }

    pub fn save_to_file(&self, path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(&self.canisters)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }
}

pub fn compute_wasm_hash(wasm: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(wasm);
    format!("{:x}", hasher.finalize())
}
