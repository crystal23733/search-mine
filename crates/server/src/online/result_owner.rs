use super::journal_postgres;
use super::postgres::{persist_result, validate_result};
use super::{ActiveMatch, FinishedMatch, PortFuture, ResultRepository, SaveResult};
use crate::auth::AuthClock;
use liar_protocol::online::OnlineError;
use sqlx::{Connection, PgConnection, PgPool};
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::AbortHandle;
use tokio::time::{Instant, MissedTickBehavior, timeout, timeout_at};
use uuid::Uuid;

const OWNER_NAMESPACE: i64 = 0x4c53_5752_0000_0000;
const DEADLINE: Duration = Duration::from_secs(2);
const QUEUE: usize = 16;
const QUEUED: u8 = 0;
const STARTED: u8 = 1;
const CANCELLED: u8 = 2;
const OBSERVED: u8 = 3;

#[derive(Clone)]
pub struct PgResultHealth {
    state: watch::Receiver<bool>,
    rejected: Arc<AtomicBool>,
}
impl PgResultHealth {
    pub fn available(&self) -> bool {
        !self.rejected.load(Ordering::SeqCst)
            && self.state.has_changed().is_ok()
            && *self.state.borrow()
    }
    fn reject(&self) {
        self.rejected.store(true, Ordering::SeqCst);
    }
    pub async fn failed(&self) {
        let mut state = self.state.clone();
        loop {
            if !self.available() {
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
            result = server => {
                if owner.available() { Ok(result) } else { Err(OnlineError::Unavailable) }
            },
        },
        None => Ok(server.await),
    }
}
#[derive(Clone, Copy)]
enum Mode {
    Normal,
    Journal(Uuid),
}
enum Command {
    Save(FinishedMatch),
    Register(ActiveMatch),
    Discard(Uuid),
}
struct Job {
    command: Command,
    recorded_at: i64,
    deadline: Instant,
    reply: oneshot::Sender<Result<SaveResult, OnlineError>>,
    progress: Option<Arc<AtomicU8>>,
}
struct RegistrationGuard {
    progress: Arc<AtomicU8>,
    health: PgResultHealth,
    abort: AbortHandle,
}
impl Drop for RegistrationGuard {
    fn drop(&mut self) {
        if self
            .progress
            .compare_exchange(QUEUED, CANCELLED, Ordering::SeqCst, Ordering::SeqCst)
            == Err(STARTED)
        {
            // An unobserved started transaction cannot safely be discarded by guessing its outcome.
            self.health.reject();
            self.abort.abort();
        }
    }
}
#[derive(Clone)]
pub struct PgResultRuntime {
    jobs: mpsc::Sender<Job>,
    health: PgResultHealth,
    clock: Arc<dyn AuthClock>,
    abort: AbortHandle,
}
async fn owner_key(connection: &mut PgConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT $1::bigint + 'online_match_results'::regclass::oid::bigint")
        .bind(OWNER_NAMESPACE)
        .fetch_one(connection)
        .await
}
pub(crate) async fn recovery_lock(connection: &mut PgConnection) -> Result<bool, OnlineError> {
    let key = owner_key(connection)
        .await
        .map_err(|_| OnlineError::Unavailable)?;
    sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
        .bind(key)
        .fetch_one(connection)
        .await
        .map_err(|_| OnlineError::Unavailable)
}
async fn claim_connection(pool: PgPool) -> Result<PgConnection, OnlineError> {
    timeout(DEADLINE, async {
        let mut connection = pool.acquire().await?.detach();
        let key = owner_key(&mut connection).await?;
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
    .map_err(|_| OnlineError::Unavailable)
}
impl PgResultRuntime {
    pub(super) fn fail_closed(&self) {
        self.health.reject();
        self.abort.abort();
    }
    pub async fn claim(pool: PgPool, clock: Arc<dyn AuthClock>) -> Result<Self, OnlineError> {
        Ok(Self::start(
            claim_connection(pool).await?,
            clock,
            Mode::Normal,
        ))
    }
    pub(super) async fn claim_journal(
        pool: PgPool,
        clock: Arc<dyn AuthClock>,
    ) -> Result<Self, OnlineError> {
        let mut connection = claim_connection(pool).await?;
        timeout(
            Duration::from_secs(30),
            journal_postgres::recover(&mut connection, clock.now()),
        )
        .await
        .map_err(|_| OnlineError::Unavailable)?
        .map_err(|_| OnlineError::Unavailable)?;
        Ok(Self::start(
            connection,
            clock,
            Mode::Journal(Uuid::new_v4()),
        ))
    }
    fn start(connection: PgConnection, clock: Arc<dyn AuthClock>, mode: Mode) -> Self {
        let (jobs, receiver) = mpsc::channel(QUEUE);
        let (state, health) = watch::channel(true);
        let task = tokio::spawn(worker(connection, receiver, state, mode));
        Self {
            jobs,
            health: PgResultHealth {
                state: health,
                rejected: Arc::new(AtomicBool::new(false)),
            },
            clock,
            abort: task.abort_handle(),
        }
    }
    pub fn health(&self) -> PgResultHealth {
        self.health.clone()
    }
    pub(super) fn register(
        &self,
        active: ActiveMatch,
    ) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(async move {
            let now = self.clock.now();
            journal_postgres::validate_active(&active, now)?;
            self.submit(Command::Register(active), now).await
        })
    }
    pub(super) fn discard(&self, id: Uuid) -> PortFuture<'_, Result<(), OnlineError>> {
        Box::pin(async move {
            if id.is_nil() {
                return Err(OnlineError::Malformed);
            }
            self.submit(Command::Discard(id), self.clock.now())
                .await
                .map(|_| ())
        })
    }
    async fn submit(&self, command: Command, recorded_at: i64) -> Result<SaveResult, OnlineError> {
        if !self.health.available() {
            return Err(OnlineError::Unavailable);
        }
        let deadline = Instant::now() + DEADLINE;
        let (reply, received) = oneshot::channel();
        let guard = matches!(&command, Command::Register(_)).then(|| RegistrationGuard {
            progress: Arc::new(AtomicU8::new(QUEUED)),
            health: self.health.clone(),
            abort: self.abort.clone(),
        });
        self.jobs
            .try_send(Job {
                command,
                recorded_at,
                deadline,
                reply,
                progress: guard.as_ref().map(|g| g.progress.clone()),
            })
            .map_err(|_| OnlineError::Unavailable)?;
        let result = timeout_at(deadline, received)
            .await
            .map_err(|_| OnlineError::Unavailable)?
            .map_err(|_| OnlineError::Unavailable)?;
        if let Some(guard) = &guard {
            guard.progress.store(OBSERVED, Ordering::SeqCst);
        }
        result
    }
}
impl ResultRepository for PgResultRuntime {
    fn save(&self, result: FinishedMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(async move {
            let now = self.clock.now();
            validate_result(&result, now)?;
            self.submit(Command::Save(result), now).await
        })
    }
}
async fn worker(
    mut connection: PgConnection,
    mut jobs: mpsc::Receiver<Job>,
    state: watch::Sender<bool>,
    mode: Mode,
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
                if job.reply.is_closed() || Instant::now() >= job.deadline {
                    continue;
                }
                if job.progress.as_ref().is_some_and(|p|
                    p.compare_exchange(QUEUED, STARTED, Ordering::SeqCst, Ordering::SeqCst).is_err()) {
                    continue;
                }
                let saved = timeout_at(job.deadline,
                    execute(&mut connection, mode, job.command, job.recorded_at)).await
                    .unwrap_or(Err(OnlineError::Unavailable));
                let unavailable = saved == Err(OnlineError::Unavailable);
                if unavailable { let _ = state.send(false); }
                let _ = job.reply.send(saved);
                if unavailable { break; }
            }
        }
    }
    let _ = state.send(false);
    let _ = timeout(DEADLINE, connection.close_hard()).await;
}

async fn execute(
    connection: &mut PgConnection,
    mode: Mode,
    command: Command,
    recorded_at: i64,
) -> Result<SaveResult, OnlineError> {
    match (mode, command) {
        (Mode::Normal, Command::Save(result)) => {
            persist_result(connection, result, recorded_at).await
        }
        (Mode::Journal(owner), Command::Save(result)) => {
            journal_postgres::finish(connection, owner, result, recorded_at).await
        }
        (Mode::Journal(owner), Command::Register(active)) => {
            journal_postgres::register(connection, owner, active, recorded_at).await
        }
        (Mode::Journal(owner), Command::Discard(id)) => {
            journal_postgres::discard(connection, owner, id).await
        }
        _ => Err(OnlineError::Malformed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dropped_worker_watch_is_failure_even_when_server_is_ready_to_return() {
        let (state, receiver) = watch::channel(true);
        let health = PgResultHealth {
            state: receiver,
            rejected: Arc::new(AtomicBool::new(false)),
        };
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
        let health = PgResultHealth {
            state: receiver,
            rejected: Arc::new(AtomicBool::new(false)),
        };
        assert_eq!(
            run_with_result_owner(Some(health.clone()), async { 7 }).await,
            Ok(7)
        );
        assert!(health.available());
        assert_eq!(run_with_result_owner(None, async { 8 }).await, Ok(8));
    }
    #[tokio::test]
    async fn explicit_fail_closed_is_visible_before_worker_abort_is_scheduled() {
        let (_state, receiver) = watch::channel(true);
        let (jobs, _received) = mpsc::channel(1);
        let worker = tokio::spawn(std::future::pending::<()>());
        let runtime = PgResultRuntime {
            jobs,
            health: PgResultHealth {
                state: receiver,
                rejected: Arc::new(AtomicBool::new(false)),
            },
            clock: Arc::new(crate::auth::SystemAuthClock),
            abort: worker.abort_handle(),
        };
        runtime.fail_closed();
        assert!(
            !runtime.health().available(),
            "Fail closed must not race successful drain completion"
        );
        assert_eq!(
            run_with_result_owner(Some(runtime.health()), async { 7 }).await,
            Err(OnlineError::Unavailable)
        );
    }
    #[tokio::test]
    async fn rejection_during_server_completion_cannot_escape_the_owner_guard() {
        let (_state, receiver) = watch::channel(true);
        let health = PgResultHealth {
            state: receiver,
            rejected: Arc::new(AtomicBool::new(false)),
        };
        let rejecting = health.clone();
        assert_eq!(
            run_with_result_owner(Some(health), async move {
                rejecting.reject();
                7
            })
            .await,
            Err(OnlineError::Unavailable)
        );
    }
}
