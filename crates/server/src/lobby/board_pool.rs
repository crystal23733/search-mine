use crate::online::PortFuture;
use liar_core::{
    board::Board,
    generator::{BoardGenerator, GenerationBudget},
    rules::RulesSnapshot,
};
use liar_protocol::online::OnlineError;
use std::{sync::Arc, time::Duration};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore, mpsc, watch};

pub trait SeedSource: Send + Sync {
    fn seed(&self) -> Result<[u8; 8], OnlineError>;
}
pub struct OsSeedSource;
impl SeedSource for OsSeedSource {
    fn seed(&self) -> Result<[u8; 8], OnlineError> {
        let mut seed = [0; 8];
        getrandom::fill(&mut seed).map_err(|_| OnlineError::Unavailable)?;
        Ok(seed)
    }
}
pub struct PreparedBoard {
    board: Board,
    rules: RulesSnapshot,
    seed: [u8; 8],
}
impl PreparedBoard {
    pub fn into_parts(self) -> (Board, RulesSnapshot, [u8; 8]) {
        (self.board, self.rules, self.seed)
    }
}
pub trait BoardSource: Send + Sync {
    fn take(&self, wait: Duration) -> PortFuture<'_, Result<PreparedBoard, OnlineError>>;
}
struct CachedBoard {
    prepared: PreparedBoard,
    _slot: OwnedSemaphorePermit,
}
pub struct BoardPool {
    receiver: Mutex<mpsc::Receiver<CachedBoard>>,
    slots: Arc<Semaphore>,
    stop: watch::Sender<bool>,
}
impl BoardPool {
    pub fn start(
        rules: RulesSnapshot,
        capacity: usize,
        workers: usize,
        seeds: Arc<dyn SeedSource>,
    ) -> Result<Self, OnlineError> {
        if !(1..=256).contains(&capacity) || !(1..=64).contains(&workers) || workers > capacity {
            return Err(OnlineError::Capacity);
        }
        rules.verify().map_err(|_| OnlineError::Malformed)?;
        let runtime =
            tokio::runtime::Handle::try_current().map_err(|_| OnlineError::Unavailable)?;
        let slots = Arc::new(Semaphore::new(capacity));
        let (sender, receiver) = mpsc::channel(capacity);
        let (stop, _) = watch::channel(false);
        let rules = Arc::new(rules);
        for _ in 0..workers {
            runtime.spawn(produce(
                rules.clone(),
                seeds.clone(),
                slots.clone(),
                sender.clone(),
                stop.subscribe(),
            ));
        }
        Ok(Self {
            receiver: Mutex::new(receiver),
            slots,
            stop,
        })
    }
}
impl Drop for BoardPool {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        self.slots.close();
    }
}
impl BoardSource for BoardPool {
    fn take(&self, wait: Duration) -> PortFuture<'_, Result<PreparedBoard, OnlineError>> {
        Box::pin(async move {
            if wait.is_zero() || wait > Duration::from_secs(2) {
                return Err(OnlineError::Malformed);
            }
            let cached =
                tokio::time::timeout(wait, async { self.receiver.lock().await.recv().await })
                    .await
                    .map_err(|_| OnlineError::Unavailable)?
                    .ok_or(OnlineError::Unavailable)?;
            Ok(cached.prepared)
        })
    }
}

async fn produce(
    rules: Arc<RulesSnapshot>,
    seeds: Arc<dyn SeedSource>,
    slots: Arc<Semaphore>,
    sender: mpsc::Sender<CachedBoard>,
    mut stop: watch::Receiver<bool>,
) {
    loop {
        if *stop.borrow() || sender.is_closed() {
            return;
        }
        let slot = tokio::select! {
            biased;
            _ = stop.changed() => return,
            permit = slots.clone().acquire_owned() => match permit {
                Ok(permit) => permit,
                Err(_) => return,
            },
        };
        let rules = rules.clone();
        let seeds = seeds.clone();
        // The actual blocking job owns capacity until it ends, including cancellation.
        let generated = tokio::task::spawn_blocking(move || {
            let seed = seeds.seed()?;
            let generated = BoardGenerator::generate(
                rules.rules.board_spec(),
                u64::from_le_bytes(seed),
                GenerationBudget::default(),
            )
            .map_err(|_| OnlineError::Unavailable)?;
            Ok::<_, OnlineError>(CachedBoard {
                prepared: PreparedBoard {
                    board: generated.board,
                    rules: (*rules).clone(),
                    seed,
                },
                _slot: slot,
            })
        })
        .await;
        if *stop.borrow() || sender.is_closed() {
            return;
        }
        if let Ok(Ok(cached)) = generated {
            tokio::select! {
                biased;
                _ = stop.changed() => return,
                result = sender.send(cached) => if result.is_err() { return; },
            }
        } else {
            tokio::select! {
                biased;
                _ = stop.changed() => return,
                _ = tokio::time::sleep(Duration::from_millis(100)) => {},
            }
        }
    }
}
