use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bitcoin::Address;
use once_cell::sync::OnceCell;
use rand::Rng as _;

use super::btc_rpc_client::BitcoinRpcClient;

pub type Exit = Arc<AtomicBool>;
static MINING: OnceCell<AtomicBool> = OnceCell::new();

/// Default mining interval
const MINER_INTERVAL: Duration = Duration::from_secs(3);

/// A miner that can mine blocks with a configured interval
pub struct Miner {
    address: Address,
    client: Arc<BitcoinRpcClient>,
    exit: Exit,
    interval: Duration,
}

impl Miner {
    /// Create a new miner
    pub fn run(address: Address, client: &Arc<BitcoinRpcClient>, exit: &Exit) -> JoinHandle<()> {
        let miner = Self {
            address,
            client: client.clone(),
            exit: exit.clone(),
            interval: MINER_INTERVAL,
        };

        thread::spawn(move || miner.__run())
    }

    fn __run(self) {
        // random wait time to avoid start mining at the same time
        let mut rng = rand::thread_rng();
        thread::sleep(Duration::from_millis(rng.gen_range(1_500..7_000)));
        // wait for MINING available (false or None); sleep with random time
        // to avoid two miners start mining at the same time
        let wait_time = Duration::from_millis(rng.gen_range(100..1000));
        while MINING
            .get()
            .is_some_and(|v| v.load(std::sync::atomic::Ordering::Relaxed))
        {
            thread::sleep(wait_time);
        }
        let started = Instant::now();
        // set MINING to true
        MINING
            .get_or_init(|| AtomicBool::new(true))
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // on panic set MINING to false
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if let Some(v) = MINING.get() {
                v.store(false, std::sync::atomic::Ordering::Relaxed);
            }

            hook(info);
        }));

        let mut last_mine = UNIX_EPOCH;
        let mut blocks_mined = 0;

        // start mining
        loop {
            if self.exit.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(v) = MINING.get() {
                    v.store(false, std::sync::atomic::Ordering::Relaxed);
                }
                println!(
                    "Released MINING. Exited after {} milliseconds. Blocks mined: {blocks_mined}",
                    started.elapsed().as_millis()
                );
                break;
            }

            if last_mine.elapsed().unwrap_or_default() >= self.interval {
                // mine a block
                if let Err(err) = self.client.generate_to_address(&self.address, 1) {
                    println!("Failed to mine a block: {:?}", err);
                } else {
                    blocks_mined += 1;
                }
                last_mine = SystemTime::now();
            }
        }
    }
}
