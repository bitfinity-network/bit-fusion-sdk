use std::time::Duration;

use bollard::Docker;
use bollard::container::LogsOptions;
use bridge_did::evm_link::EvmLink;
use candid::Principal;
use did::{BlockNumber, Bytes, H160, H256, Transaction, TransactionReceipt, U256};
use futures::StreamExt;
use reqwest::Response;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::TestEvm;
use crate::utils::error::{Result as TestResult, TestError};
use crate::utils::test_evm::EvmSide;

const BASE_PORT: u16 = 29_000;
const WRAPPED_PORT: u16 = 29_001;

/// Ganache EVM container
#[derive(Clone)]
pub struct GanacheEvm {
    chain_id: u64,
    pub rpc_url: String,
    rpc_client: reqwest::Client,
    log_exit: CancellationToken,
}

impl GanacheEvm {
    /// Run a new Ganache EVM container
    pub async fn new(side: EvmSide) -> Self {
        println!("Using Ganache EVM");

        let host_port = match side {
            EvmSide::Base => BASE_PORT,
            EvmSide::Wrapped => WRAPPED_PORT,
        };
        let rpc_url = format!("http://localhost:{host_port}");
        let chain_id = Self::get_chain_id(&rpc_url).await;
        println!("chain id: {chain_id}");

        let rpc_client = reqwest::Client::new();

        let exit = CancellationToken::new();
        tokio::spawn(Self::print_logs(side, exit.clone()));

        Self {
            chain_id,
            rpc_client,
            rpc_url,
            log_exit: exit,
        }
    }

    async fn print_logs(side: EvmSide, exit: CancellationToken) {
        let docker = Docker::connect_with_local_defaults().expect("Failed to connect to Docker");
        let image_name = match side {
            EvmSide::Base => "evm-base",
            EvmSide::Wrapped => "evm-wrapped",
        };

        let mut logs = docker.logs(
            image_name,
            Some(LogsOptions::<String> {
                follow: true,
                stdout: true,
                stderr: true,
                timestamps: true,
                ..Default::default()
            }),
        );

        loop {
            tokio::select! {
                _ = exit.cancelled() => {
                    break;
                }
                Some(Ok(line)) = logs.next() => {
                    print!("{side:?} Ganache: {line}", );
                }
            }
        }
    }

    /// Get the chain ID
    async fn get_chain_id(rpc_url: &str) -> u64 {
        let response = reqwest::Client::new()
            .post(rpc_url)
            .json(&serde_json::json!(
                {
                    "method": "eth_chainId",
                    "params": [],
                    "id": 1,
                    "jsonrpc": "2.0"
                }
            ))
            .send()
            .await
            .unwrap();

        assert!(response.status().is_success(), "Failed to get chain id");

        let body = response.json::<serde_json::Value>().await.unwrap();
        let chain_id_str = body["result"].as_str().unwrap();

        u64::from_str_radix(chain_id_str.trim_start_matches("0x"), 16).unwrap()
    }

    async fn rpc_request(&self, body: Value) -> TestResult<Response> {
        // this method with live mode is flaky, retry 10 times
        for _ in 0..10 {
            let response = match self
                .rpc_client
                .post(&self.rpc_url)
                .json(&body)
                .send()
                .await
                .map_err(|e| TestError::Ganache(format!("Failed to send request: {:?}", e)))
            {
                Ok(r) => r,
                Err(e) => {
                    println!("ganache rpc error: {:#?}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };

            if !response.status().is_success() {
                return Err(TestError::Ganache(format!(
                    "Failed to send request: {:?}",
                    response
                )));
            }

            return Ok(response);
        }

        Err(TestError::Ganache("Failed to send request".into()))
    }
}

#[async_trait::async_trait]
impl TestEvm for GanacheEvm {
    async fn stop(&self) {
        self.log_exit.cancel();
    }

    async fn chain_id(&self) -> TestResult<u64> {
        Ok(self.chain_id)
    }

    /// Get a copy of the RPC URL
    fn link(&self) -> EvmLink {
        EvmLink::Http(self.rpc_url.clone())
    }

    /// Mint native tokens to an address
    async fn mint_native_tokens(&self, address: H160, amount: U256) -> TestResult<()> {
        // mint
        self.rpc_request(serde_json::json!(
            {
                "jsonrpc": "2.0",
                "method": "evm_setAccountBalance",
                "params": [
                  address.to_hex_str(),
                  amount.to_hex_str()
                ],
                "id": 1
              }
        ))
        .await?;

        Ok(())
    }

    /// Send a raw transaction
    async fn send_raw_transaction(&self, transaction: Transaction) -> TestResult<H256> {
        let transaction = transaction.rlp_encoded_2718()?;

        let response = self
            .rpc_request(serde_json::json!(
                {
                    "method": "eth_sendRawTransaction",
                    "params": [format!("0x{}", hex::encode(transaction))],
                    "id": 1,
                    "jsonrpc": "2.0"
                }
            ))
            .await?;

        let body = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| TestError::Ganache(format!("Failed to parse response: {:?}", e)))?;
        let tx_hash_str = body["result"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Failed to get transaction hash: {:?}", body["error"]))
            .map_err(|e| TestError::Ganache(format!("Failed to get transaction hash: {:?}", e)))?;

        Ok(H256::from_hex_str(tx_hash_str).map_err(|e| {
            TestError::Ganache(format!("Failed to parse transaction hash: {:?}", e))
        })?)
    }

    /// Call a contract
    async fn eth_call(
        &self,
        from: Option<H160>,
        to: Option<H160>,
        value: Option<U256>,
        gas_limit: u64,
        gas_price: Option<U256>,
        data: Option<Bytes>,
    ) -> TestResult<Vec<u8>> {
        let response = self
            .rpc_request(serde_json::json!(
                {
                    "method": "eth_call",
                    "params": [
                        {
                            "from": from.map(|f| f.to_hex_str()),
                            "to": to.map(|t| t.to_hex_str()),
                            "value": value.map(|v| v.to_hex_str()),
                            "gas": format!("0x{:x}", gas_limit),
                            "gasPrice": gas_price.map(|gp| gp.to_hex_str()),
                            "data": data.map(|d| d.to_hex_str()),
                        },
                        "latest"
                    ],
                    "id": 1,
                    "jsonrpc": "2.0"
                }
            ))
            .await?;

        let body = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| TestError::Ganache(format!("Failed to parse response: {:?}", e)))?;
        let result = body["result"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Failed to get result: {:?}", body["error"]))
            .map_err(|e| TestError::Ganache(format!("Failed to get result: {:?}", e)))?;

        // hex to bytes
        hex::decode(result.trim_start_matches("0x"))
            .map_err(|e| TestError::Ganache(format!("Failed to parse result: {:?}", e)))
    }

    /// Get the balance of an address
    async fn eth_get_balance(&self, address: &H160, block: BlockNumber) -> TestResult<U256> {
        let response = self
            .rpc_request(serde_json::json!(
                {
                    "method": "eth_getBalance",
                    "params": [address.to_hex_str(), block.to_string().to_lowercase()],
                    "id": 1,
                    "jsonrpc": "2.0"
                }
            ))
            .await
            .unwrap();

        let body = response.json::<serde_json::Value>().await.unwrap();
        println!("body: {:#?}", body);
        let balance_str = body["result"].as_str().unwrap();

        U256::from_hex_str(balance_str)
            .map_err(|e| TestError::Ganache(format!("Failed to parse balance: {:?}", e)))
    }

    /// Get a transaction receipt
    async fn get_transaction_receipt(&self, hash: &H256) -> TestResult<Option<TransactionReceipt>> {
        let response = self
            .rpc_request(serde_json::json!(
                {
                    "method": "eth_getTransactionReceipt",
                    "params": [hash.to_hex_str()],
                    "id": 1,
                    "jsonrpc": "2.0"
                }
            ))
            .await?;

        let body = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| TestError::Ganache(format!("Failed to parse response: {:?}", e)))?;
        let result = body["result"].clone();

        if result.is_null() {
            return Ok(None);
        }

        Ok(serde_json::from_value(result)
            .map_err(|e| TestError::Ganache(format!("Failed to parse receipt: {:?}", e)))?)
    }

    /// Get the next nonce for an address
    async fn get_next_nonce(&self, address: &H160) -> TestResult<U256> {
        let response = self
            .rpc_request(serde_json::json!(
                {
                    "method": "eth_getTransactionCount",
                    "params": [address.to_hex_str(), "pending"],
                    "id": 1,
                    "jsonrpc": "2.0"
                }
            ))
            .await?;

        let body = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| TestError::Ganache(format!("Failed to parse response: {:?}", e)))?;
        let nonce_str = body["result"].as_str().ok_or_else(|| {
            TestError::Ganache(format!("Failed to get nonce: {:?}", body["error"]))
        })?;

        Ok(u64::from_str_radix(nonce_str.trim_start_matches("0x"), 16)
            .map_err(|e| TestError::Ganache(format!("Failed to parse nonce: {:?}", e)))?
            .into())
    }

    fn live(&self) -> bool {
        true
    }

    fn evm(&self) -> Principal {
        Principal::anonymous()
    }

    fn signature(&self) -> Principal {
        Principal::anonymous()
    }
}
