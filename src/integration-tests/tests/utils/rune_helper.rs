use std::time::{Duration, Instant};

use bitcoin::bip32::DerivationPath;
use bitcoin::{Address, Amount, FeeRate, PrivateKey, PublicKey, Txid};
use ord_rs::wallet::{
    CreateCommitTransactionArgsV2, CreateEdictTxArgs, LocalSigner, Runestone, ScriptType,
    TxInputInfo,
};
use ord_rs::{
    Nft, OrdTransactionBuilder, RevealTransactionArgs, SignCommitTransactionArgs, Utxo, Wallet,
};
use ordinals::{Etching, RuneId};
use serde::Deserialize;

use super::btc_rpc_client::BitcoinRpcClient;

pub struct RuneHelper<'a> {
    client: &'a BitcoinRpcClient,
    private_key: &'a PrivateKey,
    address: &'a Address,
}

#[derive(Debug, Deserialize)]
pub struct Terms {
    pub cap: u64,
    pub amount: u64,
}

impl<'a> RuneHelper<'a> {
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

    /// Etch a rune on the blockchain
    pub async fn etch(&self, commit_utxo: Utxo, etching: Etching) -> anyhow::Result<RuneId> {
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let public_key = self.private_key.public_key(&secp);

        // etch runestone
        let reveal_txid = self
            .inscribe_runestone(etching, commit_utxo, *self.private_key, public_key)
            .await?;
        println!("Reveal transaction sent: {}", reveal_txid);

        // advance the chain
        self.client.generate_to_address(self.address, 2)?;
        tokio::time::sleep(Duration::from_secs(5)).await;

        // get reveal tx
        println!("getting reveal transaction: {}", reveal_txid);
        let reveal_tx_block = self.client.get_transaction_block_info(&reveal_txid)?;

        let rune_id = RuneId::new(reveal_tx_block.height, reveal_tx_block.tx_index)
            .ok_or(anyhow::anyhow!("invalid rune id"))?;

        println!("Rune etched: {:?}", rune_id);

        // advance the chain
        self.client.generate_to_address(self.address, 5)?;
        println!("Advanced by 5 blocks; waiting 10 seconds");
        tokio::time::sleep(Duration::from_secs(10)).await;

        Ok(rune_id)
    }

    /// Inscribe runestone
    async fn inscribe_runestone(
        &self,
        etching: Etching,
        utxo: Utxo,
        private_key: PrivateKey,
        public_key: PublicKey,
    ) -> anyhow::Result<Txid> {
        let wallet = Wallet::new_with_signer(LocalSigner::new(private_key));
        let mut builder = OrdTransactionBuilder::new(public_key, ScriptType::P2TR, wallet);

        let mut dummy_inscription = Nft::new(
            Some("text/plain;charset=utf-8".as_bytes().to_vec()),
            Some(etching.rune.unwrap().to_string().as_bytes().to_vec()),
        );
        dummy_inscription.pointer = Some(vec![]);
        dummy_inscription.rune = Some(
            etching
                .rune
                .ok_or(anyhow::anyhow!("Invalid etching data; rune is missing"))?
                .commitment(),
        );

        let inputs = vec![utxo];
        // make commit tx
        let commit_tx = builder.build_commit_transaction_with_fixed_fees(
            bitcoin::Network::Regtest,
            CreateCommitTransactionArgsV2 {
                inputs: inputs.clone(),
                inscription: dummy_inscription,
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

        // ! let's wait for 6 confirmations - ord won't index under 6 confirmations
        let start = Instant::now();
        loop {
            self.client.generate_to_address(self.address, 1)?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            let confirmations = self.client.get_transaction_confirmations(&commit_txid)?;
            println!("commit transaction confirmations: {}", confirmations);
            if confirmations >= 6 {
                break;
            }
            if start.elapsed() > Duration::from_secs(60) {
                anyhow::bail!("commit transaction not confirmed after 60 seconds");
            }
        }

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
                runestone: Some(Runestone {
                    etching: Some(etching),
                    pointer: Some(1),
                    ..Default::default()
                }),
            })
            .await?;

        println!("Reveal transaction: {:?}", reveal_transaction);

        let reveal_txid = self.client.send_transaction(&reveal_transaction)?;

        Ok(reveal_txid)
    }

    /// Transfer a rune to another address
    pub async fn edict_rune(
        &self,
        utxos: Vec<Utxo>,
        rune_id: RuneId,
        destination: Address,
        amount: u128,
    ) -> anyhow::Result<Txid> {
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let wallet = Wallet::new_with_signer(LocalSigner::new(*self.private_key));
        let builder = OrdTransactionBuilder::new(
            self.private_key.public_key(&secp),
            ScriptType::P2WSH,
            wallet,
        );

        let inputs = utxos
            .into_iter()
            .map(|utxo| TxInputInfo {
                outpoint: bitcoin::OutPoint {
                    txid: utxo.id,
                    vout: utxo.index,
                },
                tx_out: bitcoin::TxOut {
                    value: utxo.amount,
                    script_pubkey: self.address.script_pubkey(),
                },
                derivation_path: DerivationPath::default(),
            })
            .collect::<Vec<_>>();

        let unsigned_tx = builder.create_edict_transaction(&CreateEdictTxArgs {
            rune: rune_id,
            inputs: inputs.clone(),
            destination,
            change_address: self.address.clone(),
            rune_change_address: self.address.clone(),
            amount,
            fee_rate: FeeRate::from_sat_per_vb(10).unwrap(),
        })?;

        let signed_tx = builder.sign_transaction(&unsigned_tx, &inputs).await?;

        let txid = self.client.send_transaction(&signed_tx)?;

        Ok(txid)
    }
}
