use super::postgres::{persist_result, validate_result};
use super::{FinishedMatch, PortFuture, ResultRepository, SaveResult};
use crate::auth::AuthClock;
use liar_protocol::online::OnlineError;
use sqlx::{Connection, PgConnection, PgPool};
use std::{future::Future, sync::Arc, time::Duration};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{Instant, MissedTickBehavior, timeout, timeout_at};

const OWNER_NAMESPACE: i64 = 0x4c53_5752_0000_0000;
const DEADLINE: Duration = Duration::from_secs(2);
const QUEUE: usize = 16;

#[derive(Clone)]
pub struct PgResultHealth {
    state: watch::Receiver<bool>,
}
impl PgResultHealth {
    pub fn available(&self) -> bool {
        self.state.has_changed().is_ok() && *self.state.borrow()
    }
    pub async fn failed(&self) {
        let mut state = self.state.clone();
        loop {
            if state.has_changed().is_err() || !*state.borrow() {
                return;
            }
            if state.changed().await.is_err() {
                return;
            }
        }
    }
}

pub async fn run_with_result_owner<F: Future>(
    owner: Option<PgResultHealth>,
    server: F,
) -> Result<F::Output, OnlineError> {
    match owner {
        Some(owner) => tokio::select! {
            biased;
            () = owner.failed() => Err(OnlineError::Unavailable),
            result = server => Ok(result),
        },
        None => Ok(server.await),
    }
}

struct SaveJob {
    result: FinishedMatch,
    recorded_at: i64,
    deadline: Instant,
    reply: oneshot::Sender<Result<SaveResult, OnlineError>>,
}

#[derive(Clone)]
pub struct PgResultRuntime {
    jobs: mpsc::Sender<SaveJob>,
    health: PgResultHealth,
    clock: Arc<dyn AuthClock>,
}
impl PgResultRuntime {
    pub async fn claim(pool: PgPool, clock: Arc<dyn AuthClock>) -> Result<Self, OnlineError> {
        let connection = timeout(DEADLINE, async {
            let mut connection = pool.acquire().await?.detach();
            let key: i64 = sqlx::query_scalar(
                "SELECT $1::bigint + 'online_match_results'::regclass::oid::bigint",
            )
            .bind(OWNER_NAMESPACE)
            .fetch_one(&mut connection)
            .await?;
            let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
                .bind(key)
                .fetch_one(&mut connection)
                .await?;
            if locked {
                Ok(connection)
            } else {
                Err(sqlx::Error::PoolClosed)
            }
        })
        .await
        .map_err(|_| OnlineError::Unavailable)?
        .map_err(|_| OnlineError::Unavailable)?;
        let (jobs, receiver) = mpsc::channel(QUEUE);
        let (state, health) = watch::channel(true);
        tokio::spawn(worker(connection, receiver, state));
        Ok(Self {
            jobs,
            health: PgResultHealth { state: health },
            clock,
        })
    }
    pub fn health(&self) -> PgResultHealth {
        self.health.clone()
    }
}
impl ResultRepository for PgResultRuntime {
    fn save(&self, result: FinishedMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(async move {
            let recorded_at = self.clock.now();
            validate_result(&result, recorded_at)?;
            if !self.health.available() {
                return Err(OnlineError::Unavailable);
            }
            let deadline = Instant::now() + DEADLINE;
            let (reply, received) = oneshot::channel();
            self.jobs
                .try_send(SaveJob {
                    result,
                    recorded_at,
                    deadline,
                    reply,
                })
                .map_err(|_| OnlineError::Unavailable)?;
            timeout_at(deadline, received)
                .await
                .map_err(|_| OnlineError::Unavailable)?
                .map_err(|_| OnlineError::Unavailable)?
        })
    }
}

async fn worker(
    mut connection: PgConnection,
    mut jobs: mpsc::Receiver<SaveJob>,
    state: watch::Sender<bool>,
) {
    let mut heartbeat = tokio::time::interval(Duration::from_secs(1));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            biased;
            _ = heartbeat.tick() => {
                if !matches!(timeout(DEADLINE, connection.ping()).await, Ok(Ok(()))) {
                    break;
                }
            }
            job = jobs.recv() => {
                let Some(job) = job else { break; };
                // Cancellation before starting SQL must have no durable effects.
                if job.reply.is_closed() || Instant::now() >= job.deadline {
                    continue;
                }
                let saved = timeout_at(job.deadline,
                    persist_result(&mut connection, job.result, job.recorded_at)).await
                    .unwrap_or(Err(OnlineError::Unavailable));
                let unavailable = saved == Err(OnlineError::Unavailable);
                if unavailable { let _ = state.send(false); }
                let _ = job.reply.send(saved);
                if unavailable { break; }
            }
        }
    }
    let _ = state.send(false);
    // Discard this raw connection, including any interrupted transaction. Never reconnect.
    let _ = timeout(DEADLINE, connection.close_hard()).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dropped_worker_watch_is_failure_even_when_server_is_ready_to_return() {
        let (state, receiver) = watch::channel(true);
        let health = PgResultHealth { state: receiver };
        drop(state);
        assert!(!health.available());
        assert_eq!(
            run_with_result_owner(Some(health), async { 7 }).await,
            Err(OnlineError::Unavailable)
        );
    }

    #[tokio::test]
    async fn normal_server_completion_preserves_success_with_or_without_owner() {
        let (_state, receiver) = watch::channel(true);
        let health = PgResultHealth { state: receiver };
        assert_eq!(
            run_with_result_owner(Some(health.clone()), async { 7 }).await,
            Ok(7)
        );
        assert!(health.available());
        assert_eq!(run_with_result_owner(None, async { 8 }).await, Ok(8));
    }
}
