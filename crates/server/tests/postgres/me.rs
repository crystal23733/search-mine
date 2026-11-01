use super::*;
use liar_protocol::me::*;
use liar_server::me::*;
use std::sync::atomic::{AtomicI64, Ordering};
struct Clock(AtomicI64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
fn owner(value: liar_server::privacy::ExportSession) -> MeSession {
    MeSession {
        account: value.account,
        revision: value.revision,
        hash: value.hash,
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn me_pages_are_self_only_newest_first_and_do_not_require_nickname_or_recent_reauthentication()
 {
    let pool = auth_pool().await;
    let own = owner(super::privacy::participant(&pool, 1000).await);
    let other = owner(super::privacy::participant(&pool, 1000).await);
    for i in 1..=21 {
        super::privacy::seed_online(&pool, own.account, Uuid::from_u128(i), 1000 + i as i64).await;
    }
    super::privacy::seed_online(&pool, other.account, Uuid::from_u128(22), 1099).await;
    let reader = PgMeReader::new(pool.clone(), Arc::new(Clock(AtomicI64::new(1500))));
    let read = reader.read(own, MeQuery::Overview).await.unwrap();
    let MePayload::Overview(view) = read.payload else {
        panic!("overview");
    };
    assert_eq!(view.account.id, own.account.to_string());
    assert!(view.account.nickname.is_none());
    assert_eq!(view.online.played, 21);
    assert_eq!(view.recent_online.len(), 3);
    assert!(!view.consent.cmp_available);
    let query = MeQuery::history(MeHistoryInput {
        v: 1,
        kind: MeKind::Online,
        opponent: MeOpponentFilter::Bot,
        cursor: None,
    })
    .unwrap();
    let MePayload::History(page) = reader.read(own, query).await.unwrap().payload else {
        panic!("history");
    };
    assert_eq!(page.total, 21);
    assert_eq!(page.items.len(), 20);
    let next = MeQuery::history(MeHistoryInput {
        v: 1,
        kind: MeKind::Online,
        opponent: MeOpponentFilter::Bot,
        cursor: page.next,
    })
    .unwrap();
    let MePayload::History(page) = reader.read(own, next).await.unwrap().payload else {
        panic!("history");
    };
    assert_eq!(page.items.len(), 1);
    assert!(page.next.is_none());
    let text = serde_json::to_string(&view).unwrap();
    for secret in [
        "secret_seed",
        "replay_digest",
        "replay_json",
        "subject_digest",
        "token_hash",
    ] {
        assert!(!text.contains(secret));
    }
    close_auth_pool(pool).await;
}

fn history(kind: MeKind, opponent: MeOpponentFilter) -> MeQuery {
    MeQuery::History {
        kind,
        opponent,
        after: None,
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn me_rejects_foreign_changed_revoked_and_expired_sessions_without_partial_data() {
    let pool = auth_pool().await;
    let own = owner(super::privacy::participant(&pool, 1000).await);
    let other = owner(super::privacy::participant(&pool, 1000).await);
    let clock = Arc::new(Clock(AtomicI64::new(1500)));
    let reader = PgMeReader::new(pool.clone(), clock.clone());
    for wrong in [
        MeSession {
            account: other.account,
            ..own
        },
        MeSession {
            revision: Uuid::new_v4(),
            ..own
        },
        MeSession {
            account: Uuid::nil(),
            ..own
        },
        MeSession {
            revision: Uuid::nil(),
            ..own
        },
    ] {
        assert!(matches!(
            reader.read(wrong, MeQuery::Overview).await,
            Err(MeError::Unauthorized)
        ));
    }
    clock.0.store(1000 + 30 * 86400, Ordering::SeqCst);
    assert!(matches!(
        reader.read(own, MeQuery::Overview).await,
        Err(MeError::Unauthorized)
    ));
    clock.0.store(1500, Ordering::SeqCst);
    sqlx::query("DELETE FROM auth_sessions WHERE id=$1")
        .bind(own.revision)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        reader.read(own, MeQuery::Overview).await,
        Err(MeError::Unauthorized)
    ));
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(other.account)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        reader.read(other, MeQuery::Overview).await,
        Err(MeError::Unauthorized)
    ));
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn me_online_filters_unknown_stats_exact_retention_and_fractional_cursor_order() {
    let pool = auth_pool().await;
    let own = owner(super::privacy::participant(&pool, 1000).await);
    let other = owner(super::privacy::participant(&pool, 1000).await);
    for i in 1..=23 {
        super::privacy::seed_online(&pool, own.account, Uuid::from_u128(i), 1000).await;
    }
    sqlx::query("UPDATE online_match_results SET recorded_at=recorded_at+(id::text::uuid= $1)::int*interval '1 microsecond',retention_started_at=to_timestamp(1000) WHERE id=$1").bind(Uuid::from_u128(1)).execute(&pool).await.unwrap();
    sqlx::query(
        "UPDATE online_match_players SET is_bot=false,account_id=$1 WHERE match_id=$2 AND seat=1",
    )
    .bind(other.account)
    .bind(Uuid::from_u128(22))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM online_match_players WHERE match_id=$1 AND seat=1")
        .bind(Uuid::from_u128(23))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE online_match_results SET reason='server_failure',end_elapsed_ms=NULL WHERE id=$1",
    )
    .bind(Uuid::from_u128(23))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE online_match_players SET outcome='abort',opened_safe=NULL,mistakes=NULL,accusations=NULL,correct_accusations=NULL WHERE match_id=$1").bind(Uuid::from_u128(23)).execute(&pool).await.unwrap();
    let clock = Arc::new(Clock(AtomicI64::new(1500)));
    let reader = PgMeReader::new(pool.clone(), clock.clone());
    for (opponent, total) in [
        (MeOpponentFilter::All, 23),
        (MeOpponentFilter::Human, 1),
        (MeOpponentFilter::Bot, 21),
    ] {
        let MePayload::History(page) = reader
            .read(own, history(MeKind::Online, opponent))
            .await
            .unwrap()
            .payload
        else {
            panic!("history")
        };
        assert_eq!(page.total, total);
        if opponent == MeOpponentFilter::All {
            let MeRecord::Online(first) = &page.items[0] else {
                panic!("online")
            };
            assert_eq!(first.result.match_id, Uuid::from_u128(1).to_string());
            assert_eq!(page.online.as_ref().unwrap().aborted, 1);
            let next = MeQuery::history(MeHistoryInput {
                v: 1,
                kind: MeKind::Online,
                opponent,
                cursor: page.next,
            })
            .unwrap();
            let MePayload::History(last) = reader.read(own, next).await.unwrap().payload else {
                panic!("history")
            };
            assert_eq!(last.items.len(), 3);
        }
    }
    super::privacy::fresh(
        &pool,
        liar_server::privacy::ExportSession {
            account: own.account,
            revision: own.revision,
            hash: own.hash,
        },
        1000 + 7775999,
    )
    .await;
    clock.0.store(1000 + 7775999, Ordering::SeqCst);
    assert!(reader.read(own, MeQuery::Overview).await.is_ok());
    clock.0.store(1000 + 7776000, Ordering::SeqCst);
    let MePayload::Overview(view) = reader.read(own, MeQuery::Overview).await.unwrap().payload
    else {
        panic!("overview")
    };
    assert_eq!(view.online.played, 0);
    assert_eq!(view.online.aborted, 0);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn me_daily_uses_accepted_receipts_and_session_current_consent_without_exporting_replays() {
    use liar_server::{consent::*, daily::*};
    let pool = auth_pool().await;
    let at = 1791331200 + 60;
    let own = owner(super::privacy::participant(&pool, at).await);
    let clock = Arc::new(Clock(AtomicI64::new(at)));
    let daily = PgDailyRepository::new(pool.clone(), clock.clone());
    daily.publish(super::daily::definition()).await.unwrap();
    // A daily completion follows the same nickname requirement as the actual game, not the read API.
    PgAuthStore::new(pool.clone())
        .nickname(own.account, Nickname::parse("Own player").unwrap())
        .await
        .unwrap();
    daily
        .submit(
            DailySession {
                account: own.account,
                hash: own.hash,
            },
            Uuid::new_v4(),
            &super::daily::evidence(),
        )
        .await
        .unwrap();
    PgConsentStore::new(pool.clone(), clock.clone())
        .withdraw(
            ConsentSession {
                account: own.account,
                revision: own.revision,
                hash: own.hash,
            },
            OptionalPurpose::Analytics,
        )
        .await
        .unwrap();
    let reader = PgMeReader::new(pool.clone(), clock.clone());
    let MePayload::Overview(view) = reader.read(own, MeQuery::Overview).await.unwrap().payload
    else {
        panic!("overview")
    };
    assert_eq!(view.daily_total, 1);
    assert_eq!(view.consent.status, MeConsentStatus::Current);
    assert_eq!(view.consent.analytics, MeChoice::Denied);
    assert!(!view.consent.cmp_available);
    assert_eq!(view.account.identities.len(), 1);
    assert_eq!(view.account.identities[0].provider, "google");
    let MePayload::History(page) = reader
        .read(own, history(MeKind::Daily, MeOpponentFilter::All))
        .await
        .unwrap()
        .payload
    else {
        panic!("history")
    };
    assert!(page.online.is_none());
    let MeRecord::Daily(receipt) = &page.items[0] else {
        panic!("daily")
    };
    assert_eq!(
        receipt.trust,
        liar_protocol::daily::DailyTrust::VerifiedCompletion
    );
    assert_eq!(receipt.verified_elapsed_ms, None);
    let text = serde_json::to_string(&page).unwrap();
    for excluded in [
        "replay_json",
        "replay_digest",
        "attempt_id",
        "rules_snapshot",
        "seed",
        "token_hash",
    ] {
        assert!(!text.contains(excluded));
    }
    // A verified-time receipt is created through the real immutable submission path.
    let timed = owner(super::privacy::participant(&pool, at).await);
    PgAuthStore::new(pool.clone())
        .nickname(timed.account, Nickname::parse("Timed player").unwrap())
        .await
        .unwrap();
    let session = DailySession {
        account: timed.account,
        hash: timed.hash,
    };
    let attempt = Uuid::new_v4();
    let evidence = super::daily::evidence();
    let time = super::daily::timing::sealed(session, attempt, &evidence, 4000);
    daily
        .submit_timed(session, attempt, &evidence, &time)
        .await
        .unwrap();
    let MePayload::History(page) = reader
        .read(timed, history(MeKind::Daily, MeOpponentFilter::All))
        .await
        .unwrap()
        .payload
    else {
        panic!("daily")
    };
    let MeRecord::Daily(receipt) = &page.items[0] else {
        panic!("daily")
    };
    assert_eq!(
        receipt.trust,
        liar_protocol::daily::DailyTrust::VerifiedTime
    );
    assert_eq!(receipt.verified_elapsed_ms, Some(4000));
    clock.0.store(at + 1, Ordering::SeqCst);
    sqlx::query("UPDATE session_consents SET expires_at=$2 WHERE session_id=$1")
        .bind(own.revision)
        .bind(at + 1)
        .execute(&pool)
        .await
        .unwrap();
    let MePayload::Overview(view) = reader.read(own, MeQuery::Overview).await.unwrap().payload
    else {
        panic!("overview")
    };
    assert_eq!(view.consent.status, MeConsentStatus::Expired);
    assert_eq!(view.consent.analytics, MeChoice::Unknown);
    close_auth_pool(pool).await;
}

async fn single(pool: &sqlx::PgPool) -> (sqlx::PgPool, i32) {
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(pool)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |conn, _| {
            let schema = schema.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(schema)
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let pid = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&pool)
        .await
        .unwrap();
    (pool, pid)
}
async fn waiting(pool: &sqlx::PgPool, pid: i32) {
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
async fn me_waiting_for_an_account_lock_resamples_time_and_rejects_revocation() {
    let pool = auth_pool().await;
    for change in 0..5 {
        let own = owner(super::privacy::participant(&pool, 1000).await);
        let clock = Arc::new(Clock(AtomicI64::new(1500)));
        let (single, pid) = single(&pool).await;
        let reader = PgMeReader::new(single.clone(), clock.clone());
        let mut held = pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
            .bind(own.account)
            .fetch_one(&mut *held)
            .await
            .unwrap();
        let task = tokio::spawn(async move { reader.read(own, MeQuery::Overview).await });
        waiting(&pool, pid).await;
        let expected = match change {
            0 => {
                clock.0.store(1600, Ordering::SeqCst);
                None
            }
            1 => {
                clock.0.store(1499, Ordering::SeqCst);
                Some(MeError::Unavailable)
            }
            2 => {
                sqlx::query("DELETE FROM auth_sessions WHERE id=$1")
                    .bind(own.revision)
                    .execute(&mut *held)
                    .await
                    .unwrap();
                Some(MeError::Unauthorized)
            }
            3 => {
                sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
                    .bind(own.account)
                    .execute(&mut *held)
                    .await
                    .unwrap();
                Some(MeError::Unauthorized)
            }
            _ => {
                sqlx::query("UPDATE auth_sessions SET id=$2 WHERE id=$1")
                    .bind(own.revision)
                    .bind(Uuid::new_v4())
                    .execute(&mut *held)
                    .await
                    .unwrap();
                Some(MeError::Unauthorized)
            }
        };
        held.commit().await.unwrap();
        let result = task.await.unwrap();
        if let Some(error) = expected {
            assert!(matches!(result,Err(value) if value==error));
        } else {
            assert_eq!(result.unwrap().observed_at, 1600);
        }
        single.close().await;
    }
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn me_deadlines_cancellation_sql_failure_and_capacity_never_report_empty_success() {
    let pool = auth_pool().await;
    let own = owner(super::privacy::participant(&pool, 1000).await);
    let (single, pid) = single(&pool).await;
    let reader = Arc::new(PgMeReader::new(
        single.clone(),
        Arc::new(Clock(AtomicI64::new(1500))),
    ));
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(own.account)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let running = reader.clone();
    let task = tokio::spawn(async move { running.read(own, MeQuery::Overview).await });
    waiting(&pool, pid).await;
    assert!(matches!(task.await.unwrap(), Err(MeError::Unavailable)));
    held.rollback().await.unwrap();
    let occupied = single.acquire().await.unwrap();
    assert!(matches!(
        reader.read(own, MeQuery::Overview).await,
        Err(MeError::Unavailable)
    ));
    drop(occupied);
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(own.account)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let running = reader.clone();
    let task = tokio::spawn(async move { running.read(own, MeQuery::Overview).await });
    waiting(&pool, pid).await;
    task.abort();
    assert!(matches!(task.await,Err(error) if error.is_cancelled()));
    held.rollback().await.unwrap();
    assert!(reader.read(own, MeQuery::Overview).await.is_ok());
    sqlx::query("ALTER TABLE online_match_results RENAME TO missing_online")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        reader.read(own, MeQuery::Overview).await,
        Err(MeError::Unavailable)
    ));
    sqlx::query("ALTER TABLE missing_online RENAME TO online_match_results")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO online_match_results(id,rules_hash,secret_seed,end_elapsed_ms,reason,recorded_at,seed_expires_at) SELECT md5(i::text)::uuid,$1,$2,243000,'timeout',to_timestamp(1000),to_timestamp(605800) FROM generate_series(1,10001) i").bind(liar_core::rules::RulesSnapshot::bundled().hash).bind([46u8;8].as_slice()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO online_match_players(match_id,seat,account_id,is_bot,outcome,opened_safe,mistakes,accusations,correct_accusations) SELECT id,0,$1,false,'draw',4,0,0,0 FROM online_match_results").bind(own.account).execute(&pool).await.unwrap();
    assert!(matches!(
        reader
            .read(own, history(MeKind::Online, MeOpponentFilter::All))
            .await,
        Err(MeError::Unavailable)
    ));
    sqlx::query("DELETE FROM online_match_results WHERE id=md5('10001')::uuid")
        .execute(&pool)
        .await
        .unwrap();
    let MePayload::History(page) = reader
        .read(own, history(MeKind::Online, MeOpponentFilter::All))
        .await
        .unwrap()
        .payload
    else {
        panic!("history")
    };
    assert_eq!(page.total, 10000);
    assert_eq!(page.items.len(), 20);
    single.close().await;
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn me_future_rows_are_excluded_and_corrupt_own_results_never_become_zero_stats() {
    let pool = auth_pool().await;
    let own = owner(super::privacy::participant(&pool, 1000).await);
    let id = Uuid::new_v4();
    super::privacy::seed_online(&pool, own.account, id, 1501).await;
    let reader = PgMeReader::new(pool.clone(), Arc::new(Clock(AtomicI64::new(1500))));
    let MePayload::Overview(view) = reader.read(own, MeQuery::Overview).await.unwrap().payload
    else {
        panic!("overview")
    };
    assert_eq!(view.online.played, 0);
    let invalid=sqlx::query("UPDATE online_match_results SET recorded_at=to_timestamp(1499),retention_started_at=to_timestamp(1501) WHERE id=$1").bind(id).execute(&pool).await;
    assert!(
        invalid.is_err(),
        "The database also forbids a future retention anchor after recording"
    );
    sqlx::query("UPDATE online_match_results SET recorded_at=to_timestamp(1499),retention_started_at=to_timestamp(1499),reason='server_failure' WHERE id=$1").bind(id).execute(&pool).await.unwrap();
    assert!(matches!(
        reader.read(own, MeQuery::Overview).await,
        Err(MeError::Unavailable)
    ));
    sqlx::query("UPDATE online_match_results SET end_elapsed_ms=NULL WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE online_match_players SET outcome='abort',opened_safe=NULL,mistakes=NULL,accusations=NULL,correct_accusations=NULL WHERE match_id=$1 AND seat=0").bind(id).execute(&pool).await.unwrap();
    let MePayload::Overview(view) = reader.read(own, MeQuery::Overview).await.unwrap().payload
    else {
        panic!("overview")
    };
    assert_eq!(view.online.aborted, 1);
    assert_eq!(view.recent_online[0].result.own, None);
    assert_eq!(view.recent_online[0].result.end_elapsed_ms, None);
    sqlx::query(
        "UPDATE online_match_players SET is_bot=false,account_id=$2 WHERE match_id=$1 AND seat=1",
    )
    .bind(id)
    .bind(own.account)
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        reader.read(own, MeQuery::Overview).await,
        Err(MeError::Unavailable)
    ));
    close_auth_pool(pool).await;
}
