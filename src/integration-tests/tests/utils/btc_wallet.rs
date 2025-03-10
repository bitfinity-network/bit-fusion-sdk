use bitcoin::key::Secp256k1;
use bitcoin::{Address, PrivateKey};

#[derive(Clone)]
pub struct BtcWallet {
    pub private_key: PrivateKey,
    pub address: Address,
}

impl BtcWallet {
    pub fn new_random() -> Self {
        use rand::Rng as _;
        let entropy = rand::thread_rng().gen::<[u8; 16]>();
        let mnemonic = bip39::Mnemonic::from_entropy(&entropy).unwrap();

        let seed = mnemonic.to_seed("");

        let private_key =
            bitcoin::PrivateKey::from_slice(&seed[..32], bitcoin::Network::Regtest).unwrap();
        let public_key = private_key.public_key(&Secp256k1::new());

        let address = Address::p2wpkh(&public_key, bitcoin::Network::Regtest).unwrap();

        Self {
            private_key,
            address,
        }
    }
}
