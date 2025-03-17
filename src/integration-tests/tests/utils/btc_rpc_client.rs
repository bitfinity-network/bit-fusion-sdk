use bitcoin::{Address, Amount, BlockHash, Transaction, Txid};
use bitcoincore_rpc::json::ScanTxOutRequest;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use ord_rs::Utxo;

/// Bitcoin rpc client
pub struct BitcoinRpcClient {
    client: Client,
}

pub struct TxBlockInfo {
    pub blockhash: BlockHash,
    pub tx_index: u32,
    pub height: u64,
}

impl BitcoinRpcClient {
    /// Initialize a new BitcoinRpcClient for tests
    pub fn test_client(wallet_name: &str) -> Self {
        let client = Client::new(
            &format!("http://localhost:18443/wallet/{wallet_name}"),
            Auth::UserPass(
                "ic-btc-integration".to_string(),
                "QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E=".to_string(),
            ),
        )
        .unwrap();

        client
            .create_wallet(wallet_name, None, None, None, None)
            .expect("failed to create wallet");

        Self { client }
    }

    /// Generate blocks
    pub fn generate_to_address(
        &self,
        wallet_address: &Address,
        count: u64,
    ) -> anyhow::Result<Vec<BlockHash>> {
        let blocks = self.client.generate_to_address(count, wallet_address)?;

        Ok(blocks)
    }

    /// Create a new wallet and return the address
    pub fn get_new_address(&self) -> anyhow::Result<Address> {
        let address = self
            .client
            .get_new_address(None, Some(bitcoincore_rpc::json::AddressType::Legacy))?
            .assume_checked();

        Ok(address)
    }

    /// Send bitcoin from client wallet to the provided address
    pub fn send_to_address(&self, to: &Address, amt: Amount) -> anyhow::Result<Txid> {
        let txid = self
            .client
            .send_to_address(to, amt, None, None, None, None, None, None)?;

        Ok(txid)
    }

    /// Get utxo owned by the provided address in the transaction
    pub fn get_utxo_by_address(&self, txid: &Txid, owner: &Address) -> anyhow::Result<Utxo> {
        let mut vout = 0;
        loop {
            println!("collecting tx outs for txid: {}, vout: {}", txid, vout);
            let tx_outs = self.client.get_tx_out(txid, vout, None)?;
            if tx_outs.is_none() {
                break;
            }
            let tx_out = tx_outs.unwrap();
            if let Some(address) = &tx_out.script_pub_key.address {
                if address == owner {
                    return Ok(Utxo {
                        id: *txid,
                        index: vout,
                        amount: tx_out.value,
                    });
                }
            }
            vout += 1;
        }

        anyhow::bail!("No tx out found for owner");
    }

    /// Send a signed transaction to the network
    pub fn send_transaction(&self, tx: &Transaction) -> anyhow::Result<Txid> {
        let txid = self.client.send_raw_transaction(tx)?;

        Ok(txid)
    }

    /// Get a transaction by its txid
    pub fn get_transaction(&self, txid: &Txid) -> anyhow::Result<Transaction> {
        let tx = self.client.get_raw_transaction(txid, None)?;

        Ok(tx)
    }

    /// Get the number of confirmations for a transaction
    pub fn get_transaction_confirmations(&self, txid: &Txid) -> anyhow::Result<u32> {
        let tx = self.client.get_raw_transaction_info(txid, None)?;

        Ok(tx.confirmations.unwrap_or_default())
    }

    /// Get block info for a transaction
    pub fn get_transaction_block_info(&self, txid: &Txid) -> anyhow::Result<TxBlockInfo> {
        let tx = self.client.get_raw_transaction_info(txid, None)?;
        let blockhash = tx.blockhash.ok_or(anyhow::anyhow!("tx not in block"))?;

        let block = self.client.get_block_info(&blockhash)?;

        Ok(TxBlockInfo {
            blockhash,
            tx_index: block
                .tx
                .iter()
                .position(|tx| tx == txid)
                .ok_or(anyhow::anyhow!("tx not found in block"))? as u32,
            height: block.height as u64,
        })
    }

    /// Get the block height
    pub fn get_block_height(&self) -> anyhow::Result<u64> {
        let block_height = self.client.get_block_count()?;

        Ok(block_height)
    }

    pub fn btc_balance(&self, address: &Address) -> anyhow::Result<Amount> {
        let descriptor = format!("addr({address})",);
        let response = self
            .client
            .scan_tx_out_set_blocking(&[ScanTxOutRequest::Single(descriptor)])?;

        let mut balance = Amount::ZERO;

        for unspent in response.unspents {
            balance += unspent.amount;
        }

        Ok(balance)
    }
}
