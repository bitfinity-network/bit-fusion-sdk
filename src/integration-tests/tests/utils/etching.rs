use bitcoin::bip32::DerivationPath;
use bitcoin::{Address, Amount, FeeRate, PrivateKey, PublicKey, Txid};
use bitcoincore_rpc::json::ListUnspentResultEntry;
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

pub struct Etcher<'a> {
    client: &'a BitcoinRpcClient,
    private_key: &'a PrivateKey,
    address: &'a Address,
}

#[derive(Debug, Deserialize)]
pub struct Terms {
    pub cap: u64,
    pub amount: u64,
}

impl<'a> Etcher<'a> {
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
    pub async fn etch(&self, etching: Etching) -> anyhow::Result<RuneId> {
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let public_key = self.private_key.public_key(&secp);

        // etch runestone
        let reveal_txid = self
            .etch_runestone(etching, self.private_key.clone(), public_key)
            .await?;
        println!("Reveal transaction sent: {}", reveal_txid);

        // advance the chain
        self.client.generate_to_address(&self.address, 1)?;

        // get reveal tx
        let reveal_tx = self.client.get_transaction(&reveal_txid)?;

        let rune_id = RuneId::new(
            reveal_tx
                .blockheight
                .map(|x| x as u64)
                .ok_or(anyhow::anyhow!("tx didn't land"))?,
            reveal_tx
                .blockindex
                .map(|x| x as u32)
                .ok_or(anyhow::anyhow!("tx didn't land"))?,
        )
        .ok_or(anyhow::anyhow!("invalid rune id"))?;

        println!("Rune etched: {:?}", rune_id);

        // edict
        let txid = self
            .edict_rune(self.private_key.clone(), public_key, rune_id)
            .await?;
        println!("Transaction sent: {}", txid);

        // advance the chain
        self.client.generate_to_address(&self.address, 1)?;

        Ok(rune_id)
    }

    async fn etch_runestone(
        &self,
        etching: Etching,
        private_key: PrivateKey,
        public_key: PublicKey,
    ) -> anyhow::Result<Txid> {
        // get utxos
        let utxo = self.get_max_utxo()?;

        let wallet = Wallet::new_with_signer(LocalSigner::new(private_key));
        let mut builder = OrdTransactionBuilder::new(public_key, ScriptType::P2WSH, wallet);

        let inputs = vec![TxInputInfo {
            outpoint: bitcoin::OutPoint {
                txid: utxo.txid,
                vout: utxo.vout,
            },
            tx_out: bitcoin::TxOut {
                value: utxo.amount,
                script_pubkey: self.address.script_pubkey(),
            },
            derivation_path: DerivationPath::default(),
        }];

        let ord_rs_inputs = vec![Utxo {
            id: utxo.txid,
            amount: utxo.amount,
            index: utxo.vout,
        }];
        // make commit tx
        let commit_tx = builder.build_commit_transaction_with_fixed_fees(
            bitcoin::Network::Regtest,
            CreateCommitTransactionArgsV2 {
                inputs: ord_rs_inputs.clone(),
                inscription: Nft::new(None, None),
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
                    inputs: ord_rs_inputs,
                    txin_script_pubkey: self.address.script_pubkey(),
                },
            )
            .await?;

        // send tx
        let commit_txid = self.client.send_transaction(&signed_commit_tx)?;
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
                    ..Default::default()
                }),
            })
            .await?;

        let reveal_txid = self.client.send_transaction(&reveal_transaction)?;

        Ok(reveal_txid)
    }

    async fn edict_rune(
        &self,
        private_key: PrivateKey,
        public_key: PublicKey,
        rune_id: RuneId,
    ) -> anyhow::Result<Txid> {
        let wallet = Wallet::new_with_signer(LocalSigner::new(private_key));
        let builder = OrdTransactionBuilder::new(public_key, ScriptType::P2WSH, wallet);

        let utxo = self.get_max_utxo()?;

        let inputs = vec![TxInputInfo {
            outpoint: bitcoin::OutPoint {
                txid: utxo.txid,
                vout: utxo.vout,
            },
            tx_out: bitcoin::TxOut {
                value: utxo.amount,
                script_pubkey: self.address.script_pubkey(),
            },
            derivation_path: DerivationPath::default(),
        }];

        let unsigned_tx = builder.create_edict_transaction(&CreateEdictTxArgs {
            rune: rune_id,
            inputs: inputs.clone(),
            destination: self.address.clone(),
            change_address: self.address.clone(),
            rune_change_address: self.address.clone(),
            amount: utxo.amount.to_sat() as u128,
            fee_rate: FeeRate::from_sat_per_vb(10).unwrap(),
        })?;

        let signed_tx = builder.sign_transaction(&unsigned_tx, &inputs).await?;

        let txid = self.client.send_transaction(&signed_tx)?;

        Ok(txid)
    }

    fn get_max_utxo(&self) -> anyhow::Result<ListUnspentResultEntry> {
        // get utxos
        let utxos = self.client.list_utxos(&self.address)?;
        // get max amount
        let utxo_by_max_amount = utxos
            .into_iter()
            .max_by_key(|utxo| utxo.amount)
            .ok_or(anyhow::anyhow!("No UTXOs found"))?;

        Ok(utxo_by_max_amount)
    }
}
