use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bitcoin::Address;

use super::btc_rpc_client::BitcoinRpcClient;

pub type Exit = Arc<AtomicBool>;

/// A miner that can mine blocks with a configured interval
pub struct Miner {
    address: Address,
    client: Arc<BitcoinRpcClient>,
    exit: Exit,
    interval: Duration,
}

impl Miner {
    /// Create a new miner
    pub fn run(
        address: Address,
        client: &Arc<BitcoinRpcClient>,
        exit: &Exit,
        interval: Duration,
    ) -> JoinHandle<()> {
        let miner = Self {
            address,
            client: client.clone(),
            exit: exit.clone(),
            interval,
        };

        thread::spawn(move || miner.__run())
    }

    fn __run(self) {
        let mut last_mine = UNIX_EPOCH;
        loop {
            if self.exit.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            if last_mine.elapsed().unwrap_or_default() >= self.interval {
                // mine a block
                if let Err(err) = self.client.generate_to_address(&self.address, 1) {
                    println!("Failed to mine a block: {:?}", err);
                }
                last_mine = SystemTime::now();
            }
        }
    }
}
