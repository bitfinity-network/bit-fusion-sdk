use std::time::{Duration, Instant};

use bitcoin::absolute::LockTime;
use bitcoin::key::Secp256k1;
use bitcoin::sighash::SighashCache;
use bitcoin::transaction::Version;
use bitcoin::{
    secp256k1, Address, Amount, OutPoint, PrivateKey, ScriptBuf, Sequence, Transaction, TxIn,
    TxOut, Txid, Witness,
};
use ord_rs::Utxo;

use super::btc_rpc_client::BitcoinRpcClient;

pub struct BtcTransferHelper<'a> {
    client: &'a BitcoinRpcClient,
    private_key: &'a PrivateKey,
    address: &'a Address,
}

impl<'a> BtcTransferHelper<'a> {
    pub fn new(
        client: &'a BitcoinRpcClient,
        private_key: &'a PrivateKey,
        address: &'a Address,
    ) -> Self {
        Self {
            client,
            private_key,
            address,
        }
    }

    /// Deploy BRC20 token
    pub async fn transfer(
        &self,
        amount: Amount,
        inputs: Vec<Utxo>,
        to: &Address,
    ) -> anyhow::Result<Txid> {
        let total_amount: Amount = inputs.iter().map(|input| input.amount).sum();
        println!("BTC transfer amount: {amount}; Input amount {total_amount}");
        let leftovers = total_amount - amount - (Amount::from_sat(1000) * inputs.len() as u64);

        let tx_in = inputs
            .iter()
            .map(|input| TxIn {
                previous_output: OutPoint {
                    txid: input.id,
                    vout: input.index,
                },
                script_sig: ScriptBuf::new(),
                sequence: Sequence::from_consensus(0xffffffff),
                witness: Witness::new(),
            })
            .collect();

        let tx_out = vec![
            TxOut {
                value: amount,
                script_pubkey: to.script_pubkey(),
            },
            TxOut {
                value: leftovers,
                script_pubkey: self.address.script_pubkey(),
            },
        ];

        let unsigned_tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: tx_in,
            output: tx_out,
        };

        let signed_tx = Self::sign_transaction(
            unsigned_tx,
            self.private_key,
            &Secp256k1::new(),
            inputs,
            &self.address.script_pubkey(),
        )?;

        // send tx
        let txid = self.client.send_transaction(&signed_tx)?;

        println!("BTC transfer transaction: {:?}", txid);

        // wait for 6 confirmations
        self.wait_for_confirmations(&txid, 6).await?;

        Ok(txid)
    }

    pub async fn wait_for_confirmations(
        &self,
        txid: &Txid,
        required_confirmations: u32,
    ) -> anyhow::Result<()> {
        // ! let's wait for 6 confirmations - ord won't index under 6 confirmations
        let start = Instant::now();
        loop {
            self.client.generate_to_address(self.address, 1)?;
            let confirmations: u32 = self.client.get_transaction_confirmations(txid)?;
            println!(
                "transfer transaction {txid} confirmations: {}",
                confirmations
            );
            if confirmations >= required_confirmations {
                break;
            }
            if start.elapsed() > Duration::from_secs(60) {
                anyhow::bail!("transfer transaction not confirmed after 60 seconds");
            }
        }

        Ok(())
    }

    fn sign_transaction(
        unsigned_tx: Transaction,
        private_key: &PrivateKey,
        secp: &Secp256k1<secp256k1::All>,
        inputs: Vec<Utxo>,
        sender_script_pubkey: &ScriptBuf,
    ) -> anyhow::Result<Transaction> {
        use bitcoin::hashes::Hash as _;

        let mut hash = SighashCache::new(unsigned_tx);

        for (index, input) in inputs.iter().enumerate() {
            let signature_hash = hash.p2wpkh_signature_hash(
                index,
                sender_script_pubkey,
                input.amount,
                bitcoin::EcdsaSighashType::All,
            )?;

            let message = secp256k1::Message::from_digest(signature_hash.to_byte_array());
            let signature = secp.sign_ecdsa(&message, &private_key.inner);

            // verify sig
            let secp_pubkey = private_key.inner.public_key(secp);
            secp.verify_ecdsa(&message, &signature, &secp_pubkey)?;
            let signature = bitcoin::ecdsa::Signature::sighash_all(signature);

            // append witness to input
            let witness = Witness::p2wpkh(&signature, &secp_pubkey);
            *hash
                .witness_mut(index)
                .ok_or(anyhow::anyhow!("index not found"))? = witness;
        }

        Ok(hash.into_transaction())
    }
}
