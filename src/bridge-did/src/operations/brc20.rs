use bitcoin::consensus::{Decodable as _, Encodable as _};
use bitcoin::Transaction;
use candid::types::{Serializer, Type};
use candid::CandidType;
use did::{H160, H256};
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use serde::{Deserialize, Deserializer, Serialize};

use crate::brc20_info::{Brc20Info, Brc20Tick};
use crate::events::MintedEventData;
use crate::order::{MintOrder, SignedOrders};

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub struct DepositRequest {
    pub amount: u128,
    pub brc20_tick: Brc20Tick,
    pub dst_address: H160,
    pub dst_token: H160,
}

/// BRC20 bridge operations
#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum Brc20BridgeOp {
    /// Deposit operations
    Deposit(Brc20BridgeDepositOp),
    /// Withdraw operations
    Withdraw(Brc20BridgeWithdrawOp),
}

/// BRC20 bridge deposit operations
#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum Brc20BridgeDepositOp {
    /// Await for deposit inputs
    AwaitInputs(DepositRequest),
    /// Await for minimum IC confirmations
    AwaitConfirmations {
        deposit: DepositRequest,
        utxos: Vec<Utxo>,
    },
    /// Sign the provided mint order
    SignMintOrder(MintOrder),
    /// Send the signed mint order to the bridge
    SendMintOrder(SignedOrders),
    /// Confirm the mint order
    ConfirmMintOrder { orders: SignedOrders, tx_id: H256 },
    /// Mint order confirmed status
    MintOrderConfirmed { data: MintedEventData },
}

/// BRC20 bridge withdraw operations
#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum Brc20BridgeWithdrawOp {
    /// Create BRC20 transfer inscription transactions
    CreateInscriptionTxs(Brc20WithdrawalPayload),
    /// Send BRC20 transfer commit transaction
    SendCommitTx {
        payload: Brc20WithdrawalPayload,
        commit_tx: DidTransaction,
        reveal_tx: DidTransaction,
        reveal_utxo: RevealUtxo,
    },
    /// Send BRC20 transfer reveal transaction
    SendRevealTx {
        payload: Brc20WithdrawalPayload,
        reveal_tx: DidTransaction,
        reveal_utxo: RevealUtxo,
    },
    /// Await for the BRC20 transfer inscription transactions to be confirmed
    AwaitInscriptionTxs {
        payload: Brc20WithdrawalPayload,
        reveal_utxo: RevealUtxo,
    },
    /// Create transfer transaction
    CreateTransferTx {
        payload: Brc20WithdrawalPayload,
        reveal_utxo: Utxo,
    },
    /// Send transfer transaction
    SendTransferTx {
        from_address: H160,
        tx: DidTransaction,
    },
    /// Transfer transaction sent
    TransferTxSent {
        from_address: H160,
        tx: DidTransaction,
    },
}

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub struct RevealUtxo {
    pub txid: [u8; 32],
    pub vout: u32,
    pub value: u64,
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct Brc20WithdrawalPayload {
    pub brc20_info: Brc20Info,
    pub amount: u128,
    pub request_ts: u64,
    pub sender: H160,
    pub dst_address: String,
}

#[derive(Debug, Clone)]
pub struct DidTransaction(pub Transaction);

impl CandidType for DidTransaction {
    fn _ty() -> Type {
        <Vec<u8> as CandidType>::_ty()
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: Serializer,
    {
        use serde::ser::Error;
        let mut bytes = vec![];
        self.0.consensus_encode(&mut bytes).map_err(Error::custom)?;

        bytes.idl_serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DidTransaction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = <Vec<u8> as Deserialize<'de>>::deserialize(deserializer)?;
        let tx =
            Transaction::consensus_decode(&mut &bytes[..]).map_err(serde::de::Error::custom)?;

        Ok(Self(tx))
    }
}

impl Serialize for DidTransaction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::Error;

        let mut bytes = vec![];
        self.0.consensus_encode(&mut bytes).map_err(Error::custom)?;
        serializer.serialize_bytes(&bytes)
    }
}

impl From<Transaction> for DidTransaction {
    fn from(value: Transaction) -> Self {
        Self(value)
    }
}

impl From<DidTransaction> for Transaction {
    fn from(value: DidTransaction) -> Self {
        value.0
    }
}
