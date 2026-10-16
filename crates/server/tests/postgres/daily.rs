use super::*;
use liar_core::rules::RulesSnapshot;
use liar_server::daily::*;
use std::sync::atomic::{AtomicI64, Ordering};

const START: i64 = 1_791_331_200;
#[path = "daily_rankings.rs"]
mod rankings;
#[path = "daily_storage.rs"]
mod storage;
#[path = "daily_time_storage.rs"]
pub(super) mod timing;
struct Clock(AtomicI64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
pub(super) fn definition() -> PublishedDaily {
    PublishedDaily::new("2026-10-07", 1, RulesSnapshot::bundled()).unwrap()
}
pub(super) fn evidence() -> ReplayEvidence {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    NativeReplayVerifier
        .verify(&definition(), &fixture["daily"]["replay"].to_string())
        .unwrap()
}
pub(super) async fn participant(pool: &sqlx::PgPool, now: i64) -> DailySession {
    let auth = PgAuthStore::new(pool.clone());
    let token = SecretToken::generate().unwrap();
    let account = auth
        .login(super::rights::account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            now,
        ))
        .await
        .unwrap()
        .account
        .id;
    auth.nickname(account, Nickname::parse("Daily Player").unwrap())
        .await
        .unwrap();
    DailySession {
        account,
        hash: token.hash(),
    }
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn first_completion_and_identical_retry_return_the_same_committed_receipt() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(START + 60)));
    let store = PgDailyRepository::new(pool.clone(), clock.clone());
    store.publish(definition()).await.unwrap();
    let session = participant(&pool, START + 60).await;
    let attempt = Uuid::new_v4();
    let verified = evidence();
    let DailySubmission::Official(first) = store.submit(session, attempt, &verified).await.unwrap()
    else {
        panic!("first completion must be official")
    };
    assert_eq!(first.metadata, *definition().metadata());
    assert_eq!(first.opened_safe, 216);
    assert_eq!(first.replayed_elapsed_ms, 3200);
    assert_eq!(first.completed_at, START + 60);
    clock.0.store(START + 120, Ordering::SeqCst);
    assert_eq!(
        store.submit(session, attempt, &verified).await.unwrap(),
        DailySubmission::Official(first.clone())
    );
    assert_eq!(
        store.own(session, definition().metadata()).await.unwrap(),
        Some(first)
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM daily_completions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn published_snapshot_is_the_accepted_committed_definition() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(START + 60)));
    let store = PgDailyRepository::new(pool.clone(), clock.clone());
    let accepted = store.publish(definition()).await.unwrap();
    assert_eq!(accepted.metadata(), definition().metadata());
    let mut rules = RulesSnapshot::bundled().rules;
    rules.duration_ms += 1;
    let proposal =
        PublishedDaily::new("2026-10-07", 1, RulesSnapshot::from_rules(rules).unwrap()).unwrap();
    assert_ne!(proposal.metadata(), accepted.metadata());
    assert_eq!(
        store.publish(proposal).await.unwrap().metadata(),
        accepted.metadata()
    );
    assert_eq!(
        store
            .definition("2026-10-07", 1)
            .await
            .unwrap()
            .unwrap()
            .rules(),
        accepted.rules()
    );
    assert!(
        sqlx::query("UPDATE daily_definitions SET rules_snapshot=rules_snapshot")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(store.definition("2026-10-06", 1).await.unwrap().is_none());
    for (date, version) in [("2026-02-30", 1), ("2026-10-07", 2)] {
        assert!(matches!(
            store.definition(date, version).await,
            Err(DailyStoreError::Invalid)
        ));
    }
    clock.0.store(START + 59, Ordering::SeqCst);
    assert!(matches!(
        store.definition("2026-10-07", 1).await,
        Err(DailyStoreError::Unavailable)
    ));
    clock.0.store(START + 8 * 86400, Ordering::SeqCst);
    assert_eq!(
        store.publish(definition()).await.unwrap().metadata(),
        accepted.metadata()
    );
    assert!(matches!(
        store
            .publish(PublishedDaily::new("2026-10-06", 1, RulesSnapshot::bundled()).unwrap())
            .await,
        Err(DailyStoreError::Expired)
    ));
    assert!(matches!(
        store
            .publish(PublishedDaily::new("2026-10-16", 1, RulesSnapshot::bundled()).unwrap())
            .await,
        Err(DailyStoreError::Expired)
    ));
    clock.0.store(0, Ordering::SeqCst);
    assert!(matches!(
        store.publish(definition()).await,
        Err(DailyStoreError::Invalid)
    ));
    assert!(matches!(
        store.definition("2026-10-07", 1).await,
        Err(DailyStoreError::Invalid)
    ));
    close_auth_pool(pool).await;
}

fn swept(published: &PublishedDaily) -> ReplayEvidence {
    use liar_core::{
        board::{CellId, ObservedCell},
        daily::DailyPuzzle,
        game::{Action, Command, Seat},
    };
    use liar_protocol::{
        daily::{DailyReplay, DailyTimedInput},
        game::{LocalInput, PublicAction},
    };
    let mut puzzle =
        DailyPuzzle::with_rules(&published.metadata().date, 1, published.rules().clone()).unwrap();
    let mut replay = DailyReplay {
        v: 1,
        metadata: published.metadata().clone(),
        inputs: vec![],
        final_time_ms: 3000,
    };
    let mut time = 3000;
    puzzle.advance(time).unwrap();
    for cell in 0..published.rules().rules.board_spec().area() as u16 {
        if puzzle.complete() {
            break;
        }
        let view = puzzle.projection();
        if view.own.cells.cell(CellId(cell)) != Some(ObservedCell::Unknown) {
            continue;
        }
        time += u64::from(view.own.stun_ms).max(50);
        let seq = replay.inputs.len() as u32 + 1;
        puzzle.apply(Command {
            id: seq.into(),
            seq: seq.into(),
            epoch: 1,
            seat: Seat::One,
            received_at: time,
            action: Action::Open(CellId(cell)),
        });
        replay.inputs.push(DailyTimedInput {
            input: LocalInput {
                v: 1,
                command_id: seq,
                client_seq: seq,
                action: PublicAction::Open { cell },
            },
            time_ms: time as u32,
        });
    }
    assert!(puzzle.complete());
    replay.final_time_ms = time as u32;
    NativeReplayVerifier
        .verify(published, &serde_json::to_string(&replay).unwrap())
        .unwrap()
}
fn official(value: DailySubmission) -> DailyReceipt {
    match value {
        DailySubmission::Official(value) => value,
        _ => panic!("expected official receipt"),
    }
}
async fn count(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM daily_completions")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn concurrent_publishers_use_one_accepted_snapshot_and_only_its_evidence() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(START + 60)));
    let store = PgDailyRepository::new(pool.clone(), clock);
    let mut rules = RulesSnapshot::bundled().rules;
    rules.duration_ms += 1;
    let other =
        PublishedDaily::new("2026-10-07", 1, RulesSnapshot::from_rules(rules).unwrap()).unwrap();
    let (a, b) = tokio::join!(store.publish(definition()), store.publish(other.clone()));
    let accepted = a.unwrap();
    assert_eq!(accepted.metadata(), b.unwrap().metadata());
    let rejected = if accepted.metadata() == definition().metadata() {
        other
    } else {
        definition()
    };
    let session = participant(&pool, START + 60).await;
    assert_eq!(
        store
            .submit(session, Uuid::new_v4(), &swept(&rejected))
            .await,
        Err(DailyStoreError::Invalid)
    );
    let saved = official(
        store
            .submit(session, Uuid::new_v4(), &swept(&accepted))
            .await
            .unwrap(),
    );
    assert_eq!(&saved.metadata, accepted.metadata());
    assert_eq!(
        store.own(session, rejected.metadata()).await,
        Err(DailyStoreError::Invalid)
    );
    assert_eq!(count(&pool).await, 1);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn identical_first_publishers_converge_across_both_unique_constraints() {
    let pool = auth_pool().await;
    for offset in 0..16 {
        let now = START + offset * 86400 + 60;
        let store = PgDailyRepository::new(pool.clone(), Arc::new(Clock(AtomicI64::new(now))));
        let date = chrono::DateTime::from_timestamp(now, 0)
            .unwrap()
            .date_naive()
            .to_string();
        let candidate = PublishedDaily::new(&date, 1, RulesSnapshot::bundled()).unwrap();
        let barrier = Arc::new(tokio::sync::Barrier::new(4));
        let mut tasks = Vec::new();
        for _ in 0..4 {
            let store = store.clone();
            let candidate = candidate.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                store.publish(candidate).await
            }));
        }
        for task in tasks {
            let accepted = task.await.unwrap();
            assert!(
                accepted.is_ok(),
                "Identical concurrent publications must converge: {accepted:?}"
            );
            assert_eq!(accepted.unwrap().metadata(), candidate.metadata());
        }
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM daily_definitions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 16);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn canonical_retry_changed_payload_changed_day_and_practice_keep_the_first_row() {
    use sha2::{Digest, Sha256};
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(START + 60)));
    let store = PgDailyRepository::new(pool.clone(), clock.clone());
    store.publish(definition()).await.unwrap();
    let session = participant(&pool, START + 60).await;
    let attempt = Uuid::new_v4();
    let verified = evidence();
    let first = official(store.submit(session, attempt, &verified).await.unwrap());
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    let source = fixture["daily"]["replay"].clone();
    let same = NativeReplayVerifier
        .verify(
            &definition(),
            &serde_json::to_string_pretty(&source).unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.submit(session, attempt, &same).await.unwrap(),
        DailySubmission::Official(first.clone())
    );
    let typed: liar_protocol::daily::DailyReplay = serde_json::from_value(source.clone()).unwrap();
    let canonical = serde_json::to_string(&typed).unwrap();
    let row = sqlx::query("SELECT replay_json::text AS replay,replay_digest,EXTRACT(EPOCH FROM replay_expires_at)::bigint AS replay_expiry,EXTRACT(EPOCH FROM record_expires_at)::bigint AS record_expiry FROM daily_completions").fetch_one(&pool).await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&row.get::<String, _>("replay")).unwrap(),
        source
    );
    assert_eq!(
        row.get::<Vec<u8>, _>("replay_digest"),
        Sha256::digest(canonical.as_bytes()).to_vec()
    );
    assert_eq!(row.get::<i64, _>("replay_expiry"), START + 60 + 604800);
    assert_eq!(row.get::<i64, _>("record_expiry"), START + 60 + 7776000);
    let mut changed = source;
    changed["final_time_ms"] = serde_json::json!(u32::MAX);
    let changed = NativeReplayVerifier
        .verify(&definition(), &changed.to_string())
        .unwrap();
    assert_eq!(changed.outcome(), verified.outcome());
    assert_eq!(
        store.submit(session, attempt, &changed).await,
        Err(DailyStoreError::Conflict)
    );
    assert_eq!(
        store
            .submit(session, Uuid::new_v4(), &changed)
            .await
            .unwrap(),
        DailySubmission::Practice(first.clone())
    );
    clock.0.store(START + 86400 + 60, Ordering::SeqCst);
    let tomorrow = store
        .publish(PublishedDaily::new("2026-10-08", 1, RulesSnapshot::bundled()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        store.submit(session, attempt, &swept(&tomorrow)).await,
        Err(DailyStoreError::Conflict)
    );
    let other = participant(&pool, START + 86400 + 60).await;
    let second = official(store.submit(other, attempt, &verified).await.unwrap());
    assert_ne!(second.id, first.id);
    assert_eq!(
        store.own(session, definition().metadata()).await.unwrap(),
        Some(first)
    );
    assert_eq!(
        store.own(other, definition().metadata()).await.unwrap(),
        Some(second)
    );
    assert_eq!(count(&pool).await, 2);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn concurrent_first_submissions_and_retries_create_one_official_completion() {
    let pool = auth_pool().await;
    let store = PgDailyRepository::new(pool.clone(), Arc::new(Clock(AtomicI64::new(START + 60))));
    store.publish(definition()).await.unwrap();
    let session = participant(&pool, START + 60).await;
    let verified = evidence();
    let (a, b) = tokio::join!(
        store.submit(session, Uuid::new_v4(), &verified),
        store.submit(session, Uuid::new_v4(), &verified)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    let first = match (a, b) {
        (DailySubmission::Official(a), DailySubmission::Practice(b))
        | (DailySubmission::Practice(b), DailySubmission::Official(a)) => {
            assert_eq!(a, b);
            a
        }
        _ => panic!("exactly one first completion"),
    };
    let (a, b) = tokio::join!(
        store.submit(session, first.attempt, &verified),
        store.submit(session, first.attempt, &verified)
    );
    assert_eq!(a.unwrap(), DailySubmission::Official(first.clone()));
    assert_eq!(b.unwrap(), DailySubmission::Official(first));
    assert_eq!(count(&pool).await, 1);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn exact_verification_retention_boundaries_do_not_extend_anchors() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(START + 60)));
    let store = PgDailyRepository::new(pool.clone(), clock.clone());
    store.publish(definition()).await.unwrap();
    let session = participant(&pool, START + 60).await;
    let verified = evidence();
    assert!(
        store
            .own(session, definition().metadata())
            .await
            .unwrap()
            .is_none()
    );
    clock.0.store(START + 604799, Ordering::SeqCst);
    let first = official(
        store
            .submit(session, Uuid::new_v4(), &verified)
            .await
            .unwrap(),
    );
    clock.0.store(START + 604800, Ordering::SeqCst);
    assert_eq!(
        store.submit(session, Uuid::new_v4(), &verified).await,
        Err(DailyStoreError::Expired)
    );
    assert_eq!(
        store
            .submit(session, first.attempt, &verified)
            .await
            .unwrap(),
        DailySubmission::Official(first.clone())
    );
    let late = participant(&pool, START + 604800).await;
    assert_eq!(
        store.submit(late, Uuid::new_v4(), &verified).await,
        Err(DailyStoreError::Expired)
    );
    // A fresh real session permits checking record retention independently from session expiry.
    let fresh_pool = &pool;
    let fresh = |now| async move {
        let token = SecretToken::generate().unwrap();
        sqlx::query("INSERT INTO auth_sessions(id,account_id,token_hash,created_at,authenticated_at,expires_at) VALUES($1,$2,$3,to_timestamp($4::double precision),to_timestamp($4::double precision),to_timestamp($4::double precision)+interval '1 day')").bind(Uuid::new_v4()).bind(session.account).bind(token.hash().as_slice()).bind(now).execute(fresh_pool).await.unwrap();
        DailySession {
            account: session.account,
            hash: token.hash(),
        }
    };
    let before = first.completed_at + 7776000 - 1;
    let session = fresh(before).await;
    clock.0.store(before, Ordering::SeqCst);
    assert_eq!(
        store.own(session, definition().metadata()).await.unwrap(),
        Some(first.clone())
    );
    clock.0.store(before + 1, Ordering::SeqCst);
    assert_eq!(
        store.own(session, definition().metadata()).await.unwrap(),
        None
    );
    assert_eq!(
        store.submit(session, first.attempt, &verified).await,
        Err(DailyStoreError::Expired)
    );
    assert_eq!(count(&pool).await, 1, "physical deletion belongs to #25");
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn revoked_missing_nickname_expired_and_deleted_accounts_cannot_collect_or_recreate() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(START + 60)));
    let store = PgDailyRepository::new(pool.clone(), clock.clone());
    let verified = evidence();
    let session = participant(&pool, START + 60).await;
    assert_eq!(
        store.submit(session, Uuid::new_v4(), &verified).await,
        Err(DailyStoreError::Invalid)
    );
    store.publish(definition()).await.unwrap();
    assert_eq!(
        store.submit(session, Uuid::nil(), &verified).await,
        Err(DailyStoreError::Invalid)
    );
    let wrong = DailySession {
        account: session.account,
        hash: [0; 32],
    };
    assert_eq!(
        store.submit(wrong, Uuid::new_v4(), &verified).await,
        Err(DailyStoreError::Unauthorized)
    );
    assert_eq!(
        store.own(wrong, definition().metadata()).await,
        Err(DailyStoreError::Unauthorized)
    );
    assert_eq!(
        store
            .own(
                DailySession {
                    account: Uuid::nil(),
                    hash: session.hash
                },
                definition().metadata()
            )
            .await,
        Err(DailyStoreError::Unauthorized)
    );
    sqlx::query("UPDATE auth_accounts SET nickname=NULL WHERE id=$1")
        .bind(session.account)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        store.submit(session, Uuid::new_v4(), &verified).await,
        Err(DailyStoreError::Unauthorized)
    );
    PgAuthStore::new(pool.clone())
        .nickname(session.account, Nickname::parse("Daily Player").unwrap())
        .await
        .unwrap();
    let first = official(
        store
            .submit(session, Uuid::new_v4(), &verified)
            .await
            .unwrap(),
    );
    PgAuthStore::new(pool.clone())
        .logout(session.hash)
        .await
        .unwrap();
    assert_eq!(
        store.submit(session, first.attempt, &verified).await,
        Err(DailyStoreError::Unauthorized)
    );
    assert_eq!(
        store.own(session, definition().metadata()).await,
        Err(DailyStoreError::Unauthorized)
    );
    let doomed = participant(&pool, START + 60).await;
    store
        .submit(doomed, Uuid::new_v4(), &verified)
        .await
        .unwrap();
    PgAuthStore::new(pool.clone())
        .erase_authorized(SessionAuthority {
            account: doomed.account,
            hash: doomed.hash,
            now: START + 60,
        })
        .await
        .unwrap();
    assert_eq!(count(&pool).await, 1);
    assert_eq!(
        store.submit(doomed, Uuid::new_v4(), &verified).await,
        Err(DailyStoreError::Unauthorized)
    );
    let expired = participant(&pool, START + 60).await;
    clock
        .0
        .store(START + 60 + SESSION_SECONDS, Ordering::SeqCst);
    assert_eq!(
        store.submit(expired, Uuid::new_v4(), &verified).await,
        Err(DailyStoreError::Unauthorized)
    );
    assert_eq!(count(&pool).await, 1);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn a_completion_before_publication_is_not_a_valid_own_receipt() {
    let pool = auth_pool().await;
    let store = PgDailyRepository::new(pool.clone(), Arc::new(Clock(AtomicI64::new(START + 60))));
    store.publish(definition()).await.unwrap();
    let session = participant(&pool, START + 60).await;
    let original = official(
        store
            .submit(session, Uuid::new_v4(), &evidence())
            .await
            .unwrap(),
    );
    let other = participant(&pool, START + 60).await;
    sqlx::query("INSERT INTO daily_completions(id,account_id,daily_id,attempt_id,replay_digest,opened_safe,mistakes,replayed_elapsed_ms,completed_at,replay_json,replay_expires_at,record_expires_at) SELECT $1,$2,daily_id,$3,replay_digest,opened_safe,mistakes,replayed_elapsed_ms,to_timestamp($4::double precision),replay_json,to_timestamp(($4+604800)::double precision),to_timestamp(($4+7776000)::double precision) FROM daily_completions WHERE id=$5")
        .bind(Uuid::new_v4()).bind(other.account).bind(Uuid::new_v4()).bind(START+59).bind(original.id).execute(&pool).await.unwrap();
    assert_eq!(
        store.own(other, definition().metadata()).await,
        Err(DailyStoreError::Unavailable)
    );
    close_auth_pool(pool).await;
}
