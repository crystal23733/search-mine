use super::*;
use liar_server::consent::*;
struct Clock;
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        1100
    }
}
struct Grant;
impl ConsentVerifier for Grant {
    async fn verify(&self, subject: Uuid, _: i64) -> Result<VerifiedConsent, ConsentError> {
        Ok(VerifiedConsent {
            subject,
            analytics: true,
            advertising: true,
            expires_at: 2000,
        })
    }
}
fn owner(v: liar_server::me::MeSession) -> ConsentSession {
    ConsentSession {
        account: v.account,
        revision: v.revision,
        hash: v.hash,
    }
}
pub(super) async fn stopped(pool: &sqlx::PgPool, pid: i32, budget: Duration) -> bool {
    tokio::time::timeout(budget, async {
        loop {
            if sqlx::query_scalar::<_, bool>(
                "SELECT NOT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1)",
            )
            .bind(pid)
            .fetch_one(pool)
            .await
            .unwrap()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok()
}
async fn operate(
    store: Arc<PgConsentStore>,
    own: ConsentSession,
    operation: u8,
) -> Result<(), ConsentError> {
    match operation {
        0 => store.read(own).await.map(|_| ()),
        1 => store.refresh(own, None, &Grant).await.map(|_| ()),
        2 => store
            .withdraw(own, OptionalPurpose::Analytics)
            .await
            .map(|_| ()),
        _ => store.withdraw_all(own).await.map(|_| ()),
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn every_consent_operation_stops_its_backend_without_releasing_the_account_blocker() {
    let pool = auth_pool().await;
    let mut held = pool.begin().await.unwrap();
    let mut tasks = vec![];
    for operation in 0..4 {
        let own = owner(super::personal::participant(&pool, 1000).await);
        sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
            .bind(own.account)
            .fetch_one(&mut *held)
            .await
            .unwrap();
        let (single, pid) = super::events::single_pool(&pool).await;
        let store = Arc::new(PgConsentStore::new(single.clone(), Arc::new(Clock)));
        let task = tokio::spawn(operate(store, own, operation));
        tasks.push((task, single, pid));
    }
    for (_, _, pid) in &tasks {
        super::events::blocked(&pool, *pid).await;
    }
    let mut observed = vec![];
    let mut connections = vec![];
    for (task, single, pid) in tasks {
        let result = tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap();
        let gone = stopped(&pool, pid, Duration::from_millis(500)).await;
        observed.push((result, gone));
        connections.push(single);
    }
    held.rollback().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM session_consents")
        .fetch_one(&pool)
        .await
        .unwrap();
    for single in connections {
        single.close().await;
    }
    close_auth_pool(pool).await;
    assert_eq!(count, 0);
    for (result, gone) in observed {
        assert_eq!(result, Err(ConsentError::Unavailable));
        assert!(
            gone,
            "The original backend must stop before its blocker is released"
        );
    }
}

struct Paused {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
impl ConsentVerifier for Paused {
    async fn verify(&self, subject: Uuid, now: i64) -> Result<VerifiedConsent, ConsentError> {
        self.entered.notify_one();
        self.release.notified().await;
        Grant.verify(subject, now).await
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn refresh_verifier_and_second_transaction_share_the_original_deadline_and_keep_denial() {
    let pool = auth_pool().await;
    let own = owner(super::personal::participant(&pool, 1000).await);
    let current = PgConsentStore::new(pool.clone(), Arc::new(Clock))
        .refresh(own, None, &Grant)
        .await
        .unwrap();
    let (single, pid) = super::events::single_pool(&pool).await;
    let store = Arc::new(PgConsentStore::new(single.clone(), Arc::new(Clock)));
    let verifier = Arc::new(Paused {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let copy = store.clone();
    let proof = verifier.clone();
    let task = tokio::spawn(async move {
        copy.refresh(own, Some(current.revision), proof.as_ref())
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), verifier.entered.notified())
        .await
        .unwrap();
    let denied = PgConsentStore::new(pool.clone(), Arc::new(Clock))
        .read(own)
        .await
        .unwrap()
        .unwrap();
    assert!(!denied.analytics && !denied.advertising);
    assert_ne!(denied.revision, current.revision);
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(own.account)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(750)).await;
    verifier.release.notify_one();
    super::events::blocked(&pool, pid).await;
    assert_eq!(task.await.unwrap(), Err(ConsentError::Unavailable));
    let gone = stopped(&pool, pid, Duration::from_millis(500)).await;
    held.rollback().await.unwrap();
    assert_eq!(
        PgConsentStore::new(pool.clone(), Arc::new(Clock))
            .read(own)
            .await
            .unwrap(),
        Some(denied)
    );
    single.close().await;
    close_auth_pool(pool).await;
    assert!(
        gone,
        "Verifier IO must not restart the second transaction's budget"
    );
}
async fn scoped_operation(
    store: Arc<PgConsentStore>,
    own: ConsentSession,
    operation: u8,
    deadline: std::time::Instant,
) -> Result<(), ConsentError> {
    match operation {
        0 => ConsentStore::read(store.as_ref(), own, deadline)
            .await
            .map(|_| ()),
        1 => ConsentStore::refresh(store.as_ref(), own, None, &Grant, deadline)
            .await
            .map(|_| ()),
        2 => ConsentStore::withdraw(store.as_ref(), own, OptionalPurpose::Analytics, deadline)
            .await
            .map(|_| ()),
        _ => ConsentStore::withdraw_all(store.as_ref(), own, deadline)
            .await
            .map(|_| ()),
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn caller_cancellation_keeps_the_original_scoped_deadline_for_each_backend() {
    let pool = auth_pool().await;
    let mut held = pool.begin().await.unwrap();
    let mut prepared = vec![];
    for operation in 0..4 {
        let own = owner(super::personal::participant(&pool, 1000).await);
        sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
            .bind(own.account)
            .fetch_one(&mut *held)
            .await
            .unwrap();
        let (single, pid) = super::events::single_pool(&pool).await;
        let store = Arc::new(PgConsentStore::new(single.clone(), Arc::new(Clock)));
        prepared.push((operation, own, single, pid, store));
    }
    let mut tasks = vec![];
    for (operation, own, single, pid, store) in prepared {
        let deadline = (tokio::time::Instant::now() + Duration::from_secs(1)).into_std();
        tasks.push((
            tokio::spawn(scoped_operation(store, own, operation, deadline)),
            single,
            pid,
        ));
    }
    for (_, _, pid) in &tasks {
        super::events::blocked(&pool, *pid).await;
    }
    for (task, _, _) in &tasks {
        task.abort();
    }
    let mut observed = vec![];
    let mut connections = vec![];
    for (task, single, pid) in tasks {
        assert!(task.await.unwrap_err().is_cancelled());
        observed.push(stopped(&pool, pid, Duration::from_millis(1500)).await);
        connections.push(single);
    }
    held.rollback().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM session_consents")
        .fetch_one(&pool)
        .await
        .unwrap();
    for single in connections {
        single.close().await;
    }
    close_auth_pool(pool).await;
    assert_eq!(count, 0);
    assert!(observed.into_iter().all(|gone| gone));
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn invalid_deadlines_do_not_poison_default_calls_and_stricter_database_limits_are_preserved()
{
    let pool = auth_pool().await;
    let own = owner(super::personal::participant(&pool, 1000).await);
    let (single, pid) = super::events::single_pool(&pool).await;
    let store = Arc::new(PgConsentStore::new(single.clone(), Arc::new(Clock)));
    for deadline in [
        (tokio::time::Instant::now() - Duration::from_secs(1)).into_std(),
        (tokio::time::Instant::now() + Duration::from_secs(3)).into_std(),
    ] {
        assert_eq!(
            ConsentStore::read(store.as_ref(), own, deadline).await,
            Err(ConsentError::Unavailable)
        );
    }
    assert_eq!(store.read(own).await, Ok(None));
    sqlx::query("SELECT set_config('transaction_timeout','300ms',false)")
        .execute(&single)
        .await
        .unwrap();
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(own.account)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let copy = store.clone();
    let deadline = (tokio::time::Instant::now() + CONSENT_REQUEST_TIMEOUT).into_std();
    let mut task =
        tokio::spawn(async move { ConsentStore::read(copy.as_ref(), own, deadline).await });
    let result = tokio::time::timeout(Duration::from_millis(1500), &mut task).await;
    if result.is_err() {
        task.abort();
        let _ = task.await;
        held.rollback().await.unwrap();
        panic!("Existing shorter DB limit must remain effective");
    }
    assert_eq!(result.unwrap().unwrap(), Err(ConsentError::Unavailable));
    let gone = stopped(&pool, pid, Duration::from_millis(500)).await;
    held.rollback().await.unwrap();
    let value = store.withdraw_all(own).await.unwrap();
    assert!(!value.analytics && !value.advertising);
    assert_eq!(store.read(own).await, Ok(Some(value)));
    let setting: String = sqlx::query_scalar("SELECT current_setting('transaction_timeout')")
        .fetch_one(&single)
        .await
        .unwrap();
    assert_eq!(setting, "0");
    single.close().await;
    close_auth_pool(pool).await;
    assert!(gone);
}
