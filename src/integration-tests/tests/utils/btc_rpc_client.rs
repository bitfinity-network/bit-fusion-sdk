use bitcoin::{Address, Amount, BlockHash, Transaction, Txid};
use bitcoincore_rpc::json::{ListUnspentResultEntry, WalletTxInfo};
use bitcoincore_rpc::{Auth, Client, RpcApi as _};

/// Bitcoin rpc client
pub struct BitcoinRpcClient {
    client: Client,
}

impl BitcoinRpcClient {
    /// Initialize a new BitcoinRpcClient for DFX tests
    pub fn dfx_test_client(wallet_name: &str) -> Self {
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
    pub fn send_to_address(&self, to: &Address, sats: u64) -> anyhow::Result<Txid> {
        let txid = self.client.send_to_address(
            to,
            Amount::from_sat(sats),
            None,
            None,
            None,
            None,
            None,
            None,
        )?;

        Ok(txid)
    }

    pub fn list_utxos(&self, address: &Address) -> anyhow::Result<Vec<ListUnspentResultEntry>> {
        let utxos = self
            .client
            .list_unspent(None, None, Some(&[address]), None, None)?;

        Ok(utxos)
    }

    pub fn send_transaction(&self, tx: &Transaction) -> anyhow::Result<Txid> {
        let txid = self.client.send_raw_transaction(tx)?;

        Ok(txid)
    }

    pub fn get_transaction(&self, txid: &Txid) -> anyhow::Result<WalletTxInfo> {
        let tx = self.client.get_transaction(txid, None)?;

        Ok(tx.info)
    }
}
