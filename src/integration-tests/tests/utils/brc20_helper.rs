use std::time::{Duration, Instant};

use bitcoin::absolute::LockTime;
use bitcoin::key::Secp256k1;
use bitcoin::sighash::SighashCache;
use bitcoin::transaction::Version;
use bitcoin::{
    secp256k1, Address, Amount, OutPoint, PrivateKey, ScriptBuf, Sequence, Transaction, TxIn,
    TxOut, Txid, Witness,
};
use brc20_bridge::brc20_info::Brc20Tick;
use ord_rs::constants::POSTAGE;
use ord_rs::wallet::{
    CreateCommitTransactionArgsV2, LocalSigner, RevealTransactionArgs, ScriptType,
};
use ord_rs::{Brc20, OrdTransactionBuilder, SignCommitTransactionArgs, Utxo, Wallet};

use super::btc_rpc_client::BitcoinRpcClient;

pub struct Brc20Helper<'a> {
    client: &'a BitcoinRpcClient,
    private_key: &'a PrivateKey,
    address: &'a Address,
}

impl<'a> Brc20Helper<'a> {
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
    pub async fn deploy(
        &self,
        tick: Brc20Tick,
        amount: u64,
        limit: u64,
        input: Utxo,
    ) -> anyhow::Result<Txid> {
        let deploy_inscription = Brc20::deploy(tick, amount, Some(limit), Some(8), None);

        self.inscribe(deploy_inscription, input).await
    }

    /// Mint BRC20 token
    pub async fn mint(&self, tick: Brc20Tick, amount: u64, input: Utxo) -> anyhow::Result<Txid> {
        let mint_inscription = Brc20::mint(tick, amount);

        self.inscribe(mint_inscription, input).await
    }

    /// Transfer BRC20 token to another address
    pub async fn transfer(
        &self,
        tick: Brc20Tick,
        amount: u64,
        recipient: Address,
        inscription_input: Utxo,
        transfer_input: Utxo,
    ) -> anyhow::Result<Txid> {
        let transfer_inscription = Brc20::transfer(tick, amount);

        let reveal_txid = self
            .inscribe(transfer_inscription, inscription_input)
            .await?;

        // wait for 6 confirmations
        self.wait_for_confirmations(&reveal_txid, 6).await?;

        // transfer to recipient
        let reveal_utxo = TxIn {
            previous_output: OutPoint {
                txid: reveal_txid,
                vout: 0,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::from_consensus(0xffffffff),
            witness: Witness::new(),
        };
        let funding_utxo = TxIn {
            previous_output: OutPoint {
                txid: transfer_input.id,
                vout: transfer_input.index,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::from_consensus(0xffffffff),
            witness: Witness::new(),
        };
        let input = vec![reveal_utxo, funding_utxo];

        let output = vec![TxOut {
            value: Amount::from_sat(POSTAGE),
            script_pubkey: recipient.script_pubkey(),
        }];

        let input_utxos = vec![
            Utxo {
                id: reveal_txid,
                index: 0,
                amount: Amount::from_sat(POSTAGE),
            },
            transfer_input,
        ];

        let unsigned_tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input,
            output,
        };

        let secp = bitcoin::secp256k1::Secp256k1::new();
        let signed_tx = Self::sign_transaction(
            unsigned_tx,
            self.private_key,
            &secp,
            input_utxos,
            &self.address.script_pubkey(),
        )?;

        let txid = self.client.send_transaction(&signed_tx)?;

        Ok(txid)
    }

    async fn inscribe(&self, inscription: Brc20, input: Utxo) -> anyhow::Result<Txid> {
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let public_key = self.private_key.public_key(&secp);

        let wallet = Wallet::new_with_signer(LocalSigner::new(*self.private_key));
        let mut builder = OrdTransactionBuilder::new(public_key, ScriptType::P2TR, wallet);

        let inputs = vec![input];

        let commit_tx = builder.build_commit_transaction_with_fixed_fees(
            bitcoin::Network::Regtest,
            CreateCommitTransactionArgsV2 {
                inputs: inputs.clone(),
                inscription,
                leftovers_recipient: self.address.clone(),
                commit_fee: Amount::from_sat(1000),
                reveal_fee: Amount::from_sat(1000),
                txin_script_pubkey: self.address.script_pubkey(),
            },
        )?;

        let signed_commit_tx = builder
            .sign_commit_transaction(
                commit_tx.unsigned_tx,
                SignCommitTransactionArgs {
                    inputs,
                    txin_script_pubkey: self.address.script_pubkey(),
                },
            )
            .await?;

        // send tx
        let commit_txid = self.client.send_transaction(&signed_commit_tx)?;

        // wait for 6 confirmations
        self.wait_for_confirmations(&commit_txid, 6).await?;

        // make reveal
        let reveal_transaction = builder
            .build_reveal_transaction(RevealTransactionArgs {
                input: Utxo {
                    id: commit_txid,
                    index: 0,
                    amount: commit_tx.reveal_balance,
                },
                recipient_address: self.address.clone(),
                redeem_script: commit_tx.redeem_script,
            })
            .await?;

        println!("Reveal transaction: {:?}", reveal_transaction);

        let reveal_txid = self.client.send_transaction(&reveal_transaction)?;

        Ok(reveal_txid)
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
            tokio::time::sleep(Duration::from_millis(100)).await;
            let confirmations: u32 = self.client.get_transaction_confirmations(txid)?;
            println!("commit transaction {txid} confirmations: {}", confirmations);
            if confirmations >= required_confirmations {
                break;
            }
            if start.elapsed() > Duration::from_secs(60) {
                anyhow::bail!("commit transaction not confirmed after 60 seconds");
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
