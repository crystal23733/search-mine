use super::*;
use liar_protocol::{daily::DailyTrust, privacy::*};
use liar_server::events::*;
use liar_server::privacy::*;
use std::sync::atomic::{AtomicI64, Ordering};
struct Clock(i64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0
    }
}
fn id(value: Uuid) -> EventId {
    EventId::parse(&value.to_string()).unwrap()
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn committed_own_source_is_captured_as_a_minimal_essential_fact() {
    let pool = auth_pool().await;
    let owner = super::privacy::participant(&pool, 1000).await;
    let match_id = Uuid::new_v4();
    super::privacy::seed_online(&pool, owner.account, match_id, 1000).await;
    let result = PgEventStore::new(pool.clone(), Arc::new(Clock(1001)))
        .capture(
            id(Uuid::new_v4()),
            EventSource::Online {
                account: id(owner.account),
                match_id: id(match_id),
            },
        )
        .await;
    assert!(
        result.is_ok(),
        "A committed own match must have a stored essential event: {result:?}"
    );
    let result = result.unwrap();
    assert!(result.inserted);
    assert_eq!(result.event.purpose(), Purpose::Essential);
    assert_eq!(result.event.occurred_at(), 1000);
    assert_eq!(result.received_at, 1001);
    let text = serde_json::to_string(&result.event).unwrap();
    for secret in [
        "seed", "board", "token", "nickname", "email", "ip", "payload", "cookie",
    ] {
        assert!(!text.contains(secret));
    }
    let reader = PgGameExportReader::new(pool.clone(), Arc::new(Clock(1001)));
    let page = reader.page(owner, ExportStream::Event, None).await.unwrap();
    assert_eq!(page.records.len(), 1);
    let ExportRecord::Event {
        event_id,
        occurred_at,
        received_at,
        fact,
    } = &page.records[0]
    else {
        panic!()
    };
    assert_eq!(event_id, &result.event.id().uuid().to_string());
    assert_eq!((*occurred_at, *received_at), (1000, 1001));
    assert!(
        matches!(fact,ExportEventFact::MatchEnd{match_id: saved,opponent:EventOpponent::Bot,..} if saved==&match_id.to_string())
    );
    close_auth_pool(pool).await;
}
fn online(account: Uuid, match_id: Uuid) -> EventSource {
    EventSource::Online {
        account: id(account),
        match_id: id(match_id),
    }
}
async fn count(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM product_events")
        .fetch_one(pool)
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn concurrent_ids_and_source_retries_return_the_original_accepted_identity() {
    let pool = auth_pool().await;
    let owner = super::privacy::participant(&pool, 1000).await;
    let game = Uuid::new_v4();
    super::privacy::seed_online(&pool, owner.account, game, 1000).await;
    let store = PgEventStore::new(pool.clone(), Arc::new(Clock(1001)));
    let event_id = id(Uuid::new_v4());
    let source = online(owner.account, game);
    let (a, b) = tokio::join!(
        store.capture(event_id, source),
        store.capture(event_id, source)
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_ne!(a.inserted, b.inserted);
    assert_eq!(a.event, b.event);
    let retry = PgEventStore::new(pool.clone(), Arc::new(Clock(1002)))
        .capture(id(Uuid::new_v4()), source)
        .await
        .unwrap();
    assert!(!retry.inserted);
    assert_eq!(retry.event.id(), event_id);
    assert_eq!(retry.received_at, 1001);
    assert_eq!(count(&pool).await, 1);
    let second = Uuid::new_v4();
    super::privacy::seed_online(&pool, owner.account, second, 1000).await;
    assert_eq!(
        store.capture(event_id, online(owner.account, second)).await,
        Err(EventStoreError::Conflict)
    );
    let other = super::privacy::participant(&pool, 1000).await;
    let third = Uuid::new_v4();
    super::privacy::seed_online(&pool, other.account, third, 1000).await;
    assert_eq!(
        store.capture(event_id, online(other.account, third)).await,
        Err(EventStoreError::Conflict)
    );
    let collision = id(Uuid::new_v4());
    let (a, b) = tokio::join!(
        store.capture(collision, online(owner.account, second)),
        store.capture(collision, online(other.account, third))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(
        matches!(a, Err(EventStoreError::Conflict)) || matches!(b, Err(EventStoreError::Conflict))
    );
    assert_eq!(count(&pool).await, 2);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn missing_foreign_uncommitted_and_duplicate_seat_sources_cannot_be_claimed() {
    let pool = auth_pool().await;
    let owner = super::privacy::participant(&pool, 1000).await;
    let other = super::privacy::participant(&pool, 1000).await;
    let store = PgEventStore::new(pool.clone(), Arc::new(Clock(1000)));
    let game = Uuid::new_v4();
    assert_eq!(
        store
            .capture(id(Uuid::new_v4()), online(owner.account, game))
            .await,
        Err(EventStoreError::Missing)
    );
    let mut held = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO online_match_results(id,rules_hash,reason,recorded_at,seed_expires_at) VALUES($1,$2,'server_failure',to_timestamp(1000),to_timestamp(2000))").bind(game).bind("a".repeat(64)).execute(&mut *held).await.unwrap();
    assert_eq!(
        store
            .capture(id(Uuid::new_v4()), online(owner.account, game))
            .await,
        Err(EventStoreError::Missing)
    );
    sqlx::query("INSERT INTO online_match_players(match_id,seat,account_id,is_bot,outcome) VALUES($1,0,$2,false,'abort')").bind(game).bind(owner.account).execute(&mut *held).await.unwrap();
    held.commit().await.unwrap();
    assert!(
        store
            .capture(id(Uuid::new_v4()), online(owner.account, game))
            .await
            .is_ok()
    );
    assert_eq!(
        store
            .capture(id(Uuid::new_v4()), online(other.account, game))
            .await,
        Err(EventStoreError::Missing)
    );
    sqlx::query("INSERT INTO online_match_players(match_id,seat,account_id,is_bot,outcome) VALUES($1,1,$2,false,'abort')").bind(game).bind(owner.account).execute(&pool).await.unwrap();
    assert_eq!(
        store
            .capture(id(Uuid::new_v4()), online(owner.account, game))
            .await,
        Err(EventStoreError::Unavailable)
    );
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn daily_events_derive_committed_date_mistakes_trust_and_own_identity() {
    let pool = auth_pool().await;
    let (owner, completion) = super::privacy::completed(&pool).await;
    let at = 1791331260;
    let store = PgEventStore::new(pool.clone(), Arc::new(Clock(at)));
    let source = EventSource::Daily {
        account: id(owner.account),
        completion_id: id(completion),
    };
    let saved = store.capture(id(Uuid::new_v4()), source).await.unwrap();
    assert_eq!(saved.event.occurred_at(), at);
    let reader = PgGameExportReader::new(pool.clone(), Arc::new(Clock(at)));
    let page = reader.page(owner, ExportStream::Event, None).await.unwrap();
    assert!(
        matches!(&page.records[0],ExportRecord::Event{fact:ExportEventFact::DailyVerified{completion_id,date,mistakes:0,trust:DailyTrust::VerifiedCompletion},..} if completion_id==&completion.to_string()&&date=="2026-10-07")
    );
    let other = super::privacy::participant(&pool, at).await;
    assert!(
        reader
            .page(other, ExportStream::Event, None)
            .await
            .unwrap()
            .records
            .is_empty()
    );
    assert_eq!(
        store
            .capture(
                id(Uuid::new_v4()),
                EventSource::Daily {
                    account: id(other.account),
                    completion_id: id(completion)
                }
            )
            .await,
        Err(EventStoreError::Missing)
    );
    sqlx::query("DELETE FROM daily_completions WHERE id=$1")
        .bind(completion)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(count(&pool).await, 0);
    assert_eq!(
        store.capture(saved.event.id(), source).await,
        Err(EventStoreError::Missing)
    );
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn original_thirty_days_future_time_and_deleted_accounts_never_extend_or_restore_raw() {
    let pool = auth_pool().await;
    let owner = super::privacy::participant(&pool, 1000).await;
    let game = Uuid::new_v4();
    super::privacy::seed_online(&pool, owner.account, game, 1000).await;
    let source = online(owner.account, game);
    let event_id = id(Uuid::new_v4());
    let expiry = 1000 + RAW_EVENT_SECONDS;
    let before = PgEventStore::new(pool.clone(), Arc::new(Clock(expiry - 1)));
    assert!(before.capture(event_id, source).await.is_ok());
    let expired = PgEventStore::new(pool.clone(), Arc::new(Clock(expiry)));
    assert_eq!(
        expired.capture(event_id, source).await,
        Err(EventStoreError::Expired)
    );
    super::privacy::fresh(&pool, owner, expiry - 1).await;
    assert_eq!(
        PgGameExportReader::new(pool.clone(), Arc::new(Clock(expiry - 1)))
            .page(owner, ExportStream::Event, None)
            .await
            .unwrap()
            .records
            .len(),
        1
    );
    assert!(
        PgGameExportReader::new(pool.clone(), Arc::new(Clock(expiry)))
            .page(owner, ExportStream::Event, None)
            .await
            .unwrap()
            .records
            .is_empty()
    );
    for now in [999, -1, MAX_EVENT_TIMESTAMP + 1] {
        assert_eq!(
            PgEventStore::new(pool.clone(), Arc::new(Clock(now)))
                .capture(id(Uuid::new_v4()), source)
                .await,
            Err(if now == 999 {
                EventStoreError::Missing
            } else {
                EventStoreError::Invalid
            })
        );
    }
    let fresh_game = Uuid::new_v4();
    super::privacy::seed_online(&pool, owner.account, fresh_game, expiry + 1).await;
    assert_eq!(
        expired
            .capture(id(Uuid::new_v4()), online(owner.account, fresh_game))
            .await,
        Err(EventStoreError::Invalid)
    );
    PgAuthStore::new(pool.clone())
        .erase_authorized(SessionAuthority {
            account: owner.account,
            hash: owner.hash,
            now: expiry,
        })
        .await
        .unwrap();
    assert_eq!(count(&pool).await, 0);
    assert_eq!(
        before.capture(event_id, source).await,
        Err(EventStoreError::Missing)
    );
    assert!(
        PgGameExportReader::new(pool.clone(), Arc::new(Clock(expiry)))
            .page(owner, ExportStream::Event, None)
            .await
            .is_err()
    );
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn essential_schema_is_immutable_and_has_no_optional_or_sensitive_payload() {
    let pool = auth_pool().await;
    let owner = super::privacy::participant(&pool, 1000).await;
    let game = Uuid::new_v4();
    super::privacy::seed_online(&pool, owner.account, game, 1000).await;
    let saved = PgEventStore::new(pool.clone(), Arc::new(Clock(1001)))
        .capture(id(Uuid::new_v4()), online(owner.account, game))
        .await
        .unwrap();
    for query in [
        "UPDATE product_events SET received_at=received_at+1",
        "UPDATE product_events SET occurred_at=occurred_at+1",
        "UPDATE product_events SET purpose='analytics'",
        "UPDATE product_events SET end_reason='clear'",
    ] {
        assert_eq!(
            sqlx::query(query)
                .execute(&pool)
                .await
                .unwrap_err()
                .as_database_error()
                .unwrap()
                .code()
                .as_deref(),
            Some("23514")
        );
    }
    sqlx::query("UPDATE product_events SET received_at=received_at")
        .execute(&pool)
        .await
        .unwrap();
    for (purpose, opponent, reason, received) in [
        ("analytics", Some("bot"), Some("timeout"), 1001),
        ("advertising", Some("bot"), Some("timeout"), 1001),
        ("essential", None, Some("timeout"), 1001),
        ("essential", Some("bot"), None, 1001),
        ("essential", Some("bad"), Some("timeout"), 1001),
        (
            "essential",
            Some("bot"),
            Some("timeout"),
            1000 + RAW_EVENT_SECONDS,
        ),
    ] {
        let error=sqlx::query("INSERT INTO product_events(id,account_id,purpose,occurred_at,received_at,online_match_id,opponent_type,end_reason) VALUES($1,$2,$3,1000,$4,$5,$6,$7)").bind(Uuid::new_v4()).bind(owner.account).bind(purpose).bind(received).bind(game).bind(opponent).bind(reason).execute(&pool).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }
    let columns:Vec<String>=sqlx::query_scalar("SELECT column_name FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='product_events'").fetch_all(&pool).await.unwrap();
    assert_eq!(columns.len(), 12);
    for forbidden in ["seed", "payload", "token", "ip", "nickname", "session_id"] {
        assert!(!columns.iter().any(|c| c.contains(forbidden)));
    }
    sqlx::query("UPDATE online_match_results SET reason='forfeit' WHERE id=$1")
        .bind(game)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        PgEventStore::new(pool.clone(), Arc::new(Clock(1002)))
            .capture(saved.event.id(), online(owner.account, game))
            .await,
        Err(EventStoreError::Conflict)
    );
    assert_eq!(count(&pool).await, 1);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn own_event_pages_are_bounded_ordered_and_do_not_export_another_account() {
    let pool = auth_pool().await;
    let owner = super::privacy::participant(&pool, 1000).await;
    let other = super::privacy::participant(&pool, 1000).await;
    let store = PgEventStore::new(pool.clone(), Arc::new(Clock(1001)));
    for n in 1..=52 {
        let account = if n == 52 {
            other.account
        } else {
            owner.account
        };
        let game = Uuid::new_v4();
        super::privacy::seed_online(&pool, account, game, 1000).await;
        store
            .capture(id(Uuid::from_u128(n)), online(account, game))
            .await
            .unwrap();
    }
    let reader = PgGameExportReader::new(pool.clone(), Arc::new(Clock(1001)));
    let first = reader.page(owner, ExportStream::Event, None).await.unwrap();
    assert_eq!(first.records.len(), 50);
    assert_eq!(first.next, Some(Uuid::from_u128(50).to_string()));
    let second = reader
        .page(owner, ExportStream::Event, Some(Uuid::from_u128(50)))
        .await
        .unwrap();
    assert_eq!(second.records.len(), 1);
    assert_eq!(second.next, None);
    assert!(
        reader
            .page(owner, ExportStream::Event, Some(Uuid::from_u128(52)))
            .await
            .unwrap()
            .records
            .is_empty()
    );
    let wrong = ExportSession {
        revision: Uuid::new_v4(),
        ..owner
    };
    assert_eq!(
        reader.page(wrong, ExportStream::Event, None).await,
        Err(ExportError::Unauthorized)
    );
    assert_eq!(
        PgGameExportReader::new(pool.clone(), Arc::new(Clock(1300)))
            .page(owner, ExportStream::Event, None)
            .await,
        Err(ExportError::Reauth)
    );
    sqlx::query("DELETE FROM auth_sessions WHERE id=$1")
        .bind(owner.revision)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        reader.page(owner, ExportStream::Event, None).await,
        Err(ExportError::Unauthorized)
    );
    close_auth_pool(pool).await;
}
struct MovingClock(AtomicI64);
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn a_sealed_time_completion_produces_the_verified_time_fact_without_copying_replay() {
    use liar_server::daily::*;
    let pool = auth_pool().await;
    let owner = super::privacy::participant(&pool, 1791331260).await;
    PgAuthStore::new(pool.clone())
        .nickname(owner.account, Nickname::parse("Timed Event").unwrap())
        .await
        .unwrap();
    let definition =
        PublishedDaily::new("2026-10-07", 1, liar_core::rules::RulesSnapshot::bundled()).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    let evidence = NativeReplayVerifier
        .verify(&definition, &fixture["daily"]["replay"].to_string())
        .unwrap();
    let repository = PgDailyRepository::new(pool.clone(), Arc::new(Clock(1791331260)));
    repository.publish(definition).await.unwrap();
    let attempt = Uuid::new_v4();
    let session = DailySession {
        account: owner.account,
        hash: owner.hash,
    };
    let time = super::daily::timing::sealed(session, attempt, &evidence, 9000);
    let DailySubmission::Official(receipt) = repository
        .submit_timed(session, attempt, &evidence, &time)
        .await
        .unwrap()
    else {
        panic!()
    };
    let source = EventSource::Daily {
        account: id(owner.account),
        completion_id: id(receipt.id),
    };
    PgEventStore::new(pool.clone(), Arc::new(Clock(1791331260)))
        .capture(id(Uuid::new_v4()), source)
        .await
        .unwrap();
    let page = PgGameExportReader::new(pool.clone(), Arc::new(Clock(1791331260)))
        .page(owner, ExportStream::Event, None)
        .await
        .unwrap();
    assert!(matches!(
        &page.records[0],
        ExportRecord::Event {
            fact: ExportEventFact::DailyVerified {
                trust: DailyTrust::VerifiedTime,
                ..
            },
            ..
        }
    ));
    assert!(!serde_json::to_string(&page).unwrap().contains("replay"));
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn an_invalid_daily_receipt_cannot_be_relabelled_as_a_verified_time_event() {
    let pool = auth_pool().await;
    let (owner, completion) = super::privacy::completed(&pool).await;
    sqlx::query("ALTER TABLE daily_completions DISABLE TRIGGER daily_completion_immutable")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE daily_completions SET verified_elapsed_ms=1 WHERE id=$1")
        .bind(completion)
        .execute(&pool)
        .await
        .unwrap();
    let reader = PgGameExportReader::new(pool.clone(), Arc::new(Clock(1791331260)));
    assert_eq!(
        reader.page(owner, ExportStream::Daily, None).await,
        Err(ExportError::Unavailable)
    );
    let captured = PgEventStore::new(pool.clone(), Arc::new(Clock(1791331260)))
        .capture(
            id(Uuid::new_v4()),
            EventSource::Daily {
                account: id(owner.account),
                completion_id: id(completion),
            },
        )
        .await;
    assert_eq!(captured, Err(EventStoreError::Unavailable));
    close_auth_pool(pool).await;
}
impl AuthClock for MovingClock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
async fn single_pool(pool: &sqlx::PgPool) -> (sqlx::PgPool, i32) {
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(pool)
        .await
        .unwrap();
    let single = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |c, _| {
            let schema = schema.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(schema)
                    .execute(c)
                    .await?;
                Ok(())
            })
        })
        .connect(&env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let pid = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&single)
        .await
        .unwrap();
    (single, pid)
}
async fn blocked(pool: &sqlx::PgPool, pid: i32) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if sqlx::query_scalar::<_, bool>("SELECT cardinality(pg_blocking_pids($1))>0")
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
    .unwrap();
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn blocked_capture_observes_deletion_clock_rollback_expiry_and_actual_io_timeout() {
    let pool = auth_pool().await;
    for change in 0..4 {
        let owner = super::privacy::participant(&pool, 1000).await;
        let game = Uuid::new_v4();
        super::privacy::seed_online(&pool, owner.account, game, 1000).await;
        let (single, pid) = single_pool(&pool).await;
        let clock = Arc::new(MovingClock(AtomicI64::new(1000)));
        let store = PgEventStore::new(single.clone(), clock.clone());
        let mut held = pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
            .bind(owner.account)
            .fetch_one(&mut *held)
            .await
            .unwrap();
        let task = tokio::spawn(async move {
            store
                .capture(id(Uuid::new_v4()), online(owner.account, game))
                .await
        });
        blocked(&pool, pid).await;
        let expected = match change {
            0 => {
                sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
                    .bind(owner.account)
                    .execute(&mut *held)
                    .await
                    .unwrap();
                EventStoreError::Missing
            }
            1 => {
                clock.0.store(999, Ordering::SeqCst);
                EventStoreError::Invalid
            }
            2 => {
                clock.0.store(1000 + RAW_EVENT_SECONDS, Ordering::SeqCst);
                EventStoreError::Expired
            }
            _ => {
                assert_eq!(
                    tokio::time::timeout(Duration::from_secs(3), task)
                        .await
                        .unwrap()
                        .unwrap(),
                    Err(EventStoreError::Unavailable)
                );
                held.rollback().await.unwrap();
                assert_eq!(count(&pool).await, 0);
                single.close().await;
                continue;
            }
        };
        held.commit().await.unwrap();
        assert_eq!(task.await.unwrap(), Err(expected));
        assert_eq!(count(&pool).await, 0);
        single.close().await;
    }
    close_auth_pool(pool).await;
}
