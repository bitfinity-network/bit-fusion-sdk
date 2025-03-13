mod image;

use std::sync::Arc;

use bridge_did::evm_link::EvmLink;
use did::{BlockNumber, Bytes, Transaction, TransactionReceipt, H160, H256, U256};
use reqwest::Response;
use serde_json::Value;
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;

use self::image::Ganache as GanacheImage;
use super::TestEvm;
use crate::utils::error::{Result as TestResult, TestError};

/// Ganache EVM container
#[derive(Clone)]
pub struct GanacheEvm {
    chain_id: u64,
    #[allow(dead_code)]
    container: Arc<ContainerAsync<GanacheImage>>,
    pub rpc_url: String,
    rpc_client: reqwest::Client,
}

impl GanacheEvm {
    /// Run a new Ganache EVM container
    pub async fn run() -> Self {
        let container = GanacheImage.start().await.expect("Failed to start Ganache");
        let host_port = container
            .get_host_port_ipv4(8545)
            .await
            .expect("Failed to get host port for Ganache");
        let rpc_url = format!("http://localhost:{host_port}");
        let chain_id = Self::get_chain_id(&rpc_url).await;

        let rpc_client = reqwest::Client::new();

        Self {
            chain_id,
            container: Arc::new(container),
            rpc_client,
            rpc_url,
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
        dbg!("Sending request: {:#?}", &body);

        let response = self
            .rpc_client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| TestError::Ganache(format!("Failed to send request: {:?}", e)))?;

        if !response.status().is_success() {
            return Err(TestError::Ganache(format!(
                "Failed to send request: {:?}",
                response
            )));
        }

        Ok(response)
    }
}

#[async_trait::async_trait]
impl TestEvm for GanacheEvm {
    async fn eth_chain_id(&self) -> TestResult<u64> {
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
    ) -> TestResult<String> {
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

        Ok(result.to_string())
    }

    /// Get the balance of an address
    async fn eth_get_balance(&self, address: &H160, block: BlockNumber) -> TestResult<U256> {
        let response = self
            .rpc_request(serde_json::json!(
                {
                    "method": "eth_getBalance",
                    "params": [address.to_hex_str(), block.to_string()],
                    "id": 1,
                    "jsonrpc": "2.0"
                }
            ))
            .await
            .unwrap();

        let body = response.json::<serde_json::Value>().await.unwrap();
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
}
