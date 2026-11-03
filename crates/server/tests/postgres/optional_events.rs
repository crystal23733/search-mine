use super::*;
use liar_server::{consent::*, events::*};
use std::sync::atomic::{AtomicI64, Ordering};
struct Clock(AtomicI64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
struct Grant {
    analytics: bool,
    advertising: bool,
}
impl ConsentVerifier for Grant {
    async fn verify(&self, subject: Uuid, _: i64) -> Result<VerifiedConsent, ConsentError> {
        Ok(VerifiedConsent {
            subject,
            analytics: self.analytics,
            advertising: self.advertising,
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
fn draft(event_id: Uuid, tab: Uuid, advertising: bool, at: i64) -> ClientDraft {
    decode_client(&serde_json::to_vec(&serde_json::json!({"event_version":1,"event_id":event_id.to_string(),"session_key":tab.to_string(),"occurred_at":at,"locale":"ko","event":if advertising {serde_json::json!({"type":"ad_request","placement":"menu"})}else{serde_json::json!({"type":"landing_view","route":"home"})}})).unwrap()).unwrap()
}
fn input(owner: ConsentSession, revision: Uuid) -> OptionalCapture {
    OptionalCapture {
        session: owner,
        expected_revision: revision,
        draft: draft(Uuid::new_v4(), Uuid::new_v4(), false, 1100),
    }
}
async fn rows(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM optional_product_events")
        .fetch_one(pool)
        .await
        .unwrap()
}
async fn backend_stopped(pool: &sqlx::PgPool, pid: i32, budget: Duration) {
    tokio::time::timeout(budget, async {
        loop {
            let gone: bool = sqlx::query_scalar(
                "SELECT NOT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1)",
            )
            .bind(pid)
            .fetch_one(pool)
            .await
            .unwrap();
            if gone {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("The backend must end without releasing its blocker after the original deadline");
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn current_purpose_and_atomic_duplicate_receipts_never_reuse_withdrawn_permission() {
    let pool = auth_pool().await;
    let own = owner(super::personal::participant(&pool, 1000).await);
    let clock = Arc::new(Clock(AtomicI64::new(1100)));
    let consent = PgConsentStore::new(pool.clone(), clock.clone());
    let granted = consent
        .refresh(
            own,
            None,
            &Grant {
                analytics: true,
                advertising: false,
            },
        )
        .await
        .unwrap();
    let store = PgOptionalEventStore::new(pool.clone(), clock.clone());
    let write = input(own, granted.revision);
    let (a, b) = tokio::join!(store.capture(write.clone()), store.capture(write.clone()));
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(usize::from(a.inserted) + usize::from(b.inserted), 1);
    assert_eq!(a.event, b.event);
    assert_eq!(a.received_at, 1100);
    let json = serde_json::to_value(&a.event).unwrap();
    assert_ne!(json["subject"]["id"], own.revision.to_string());
    assert_eq!(json["subject"]["type"], "session");
    clock.0.store(1200, Ordering::SeqCst);
    let repeat = store.capture(write.clone()).await.unwrap();
    assert!(!repeat.inserted);
    assert_eq!(repeat.received_at, 1100);
    let mut changed = write.clone();
    // A different valid analytics payload with the same ID must conflict.
    changed.draft=decode_client(&serde_json::to_vec(&serde_json::json!({"event_version":1,"event_id":a.event.id(),"session_key":json["subject"]["id"],"occurred_at":1101,"locale":"ko","event":{"type":"landing_view","route":"daily"}})).unwrap()).unwrap();
    assert_eq!(
        store.capture(changed).await,
        Err(OptionalEventError::Conflict)
    );
    let mut ads = write.clone();
    ads.draft = draft(Uuid::new_v4(), Uuid::new_v4(), true, 1100);
    assert_eq!(store.capture(ads).await, Err(OptionalEventError::Consent));
    let denied = consent
        .withdraw(own, OptionalPurpose::Analytics)
        .await
        .unwrap();
    assert_eq!(
        store.capture(write.clone()).await,
        Err(OptionalEventError::Consent)
    );
    let mut denied_write = write;
    denied_write.expected_revision = denied.revision;
    assert_eq!(
        store.capture(denied_write).await,
        Err(OptionalEventError::Consent)
    );
    assert_eq!(rows(&pool).await, 1);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn unknown_foreign_or_disabled_consent_does_not_create_optional_or_essential_events() {
    let pool = auth_pool().await;
    let own = owner(super::personal::participant(&pool, 1000).await);
    let other = owner(super::personal::participant(&pool, 1000).await);
    let clock = Arc::new(Clock(AtomicI64::new(1100)));
    let consent = PgConsentStore::new(pool.clone(), clock.clone());
    let store = PgOptionalEventStore::new(pool.clone(), clock);
    assert_eq!(
        store.capture(input(own, Uuid::new_v4())).await,
        Err(OptionalEventError::Consent)
    );
    assert_eq!(
        consent.refresh(own, None, &DisabledConsentVerifier).await,
        Err(ConsentError::Unavailable)
    );
    let granted = consent
        .refresh(
            own,
            None,
            &Grant {
                analytics: true,
                advertising: true,
            },
        )
        .await
        .unwrap();
    let mut write = input(own, granted.revision);
    write.session.hash = other.hash;
    assert_eq!(
        store.capture(write).await,
        Err(OptionalEventError::Unauthorized)
    );
    let mut write = input(own, granted.revision);
    write.session.revision = other.revision;
    assert_eq!(
        store.capture(write).await,
        Err(OptionalEventError::Unauthorized)
    );
    assert_eq!(
        store.capture(input(other, granted.revision)).await,
        Err(OptionalEventError::Consent)
    );
    assert_eq!(rows(&pool).await, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM product_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn actual_withdrawal_and_optional_capture_serialize_in_both_lock_orders() {
    let pool = auth_pool().await;
    for write_first in [false, true] {
        let own = owner(super::personal::participant(&pool, 1000).await);
        let clock = Arc::new(Clock(AtomicI64::new(1100)));
        let first = PgConsentStore::new(pool.clone(), clock.clone())
            .refresh(
                own,
                None,
                &Grant {
                    analytics: true,
                    advertising: true,
                },
            )
            .await
            .unwrap();
        let (writes, write_pid) = super::events::single_pool(&pool).await;
        let (withdrawals, withdraw_pid) = super::events::single_pool(&pool).await;
        let mut held = pool.begin().await.unwrap();
        if write_first {
            sqlx::query("SELECT id FROM auth_sessions WHERE id=$1 FOR KEY SHARE")
                .bind(own.revision)
                .fetch_one(&mut *held)
                .await
                .unwrap();
        } else {
            sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
                .bind(own.account)
                .fetch_one(&mut *held)
                .await
                .unwrap();
        }
        let write = input(own, first.revision);
        let consent = PgConsentStore::new(withdrawals.clone(), clock.clone());
        let store = PgOptionalEventStore::new(writes.clone(), clock.clone());
        let (saved, denied) = if write_first {
            let saved = tokio::spawn(async move { store.capture(write).await });
            super::events::blocked(&pool, write_pid).await;
            let denied = tokio::spawn(async move { consent.withdraw_all(own).await });
            super::events::blocked(&pool, withdraw_pid).await;
            (saved, denied)
        } else {
            let denied = tokio::spawn(async move { consent.withdraw_all(own).await });
            super::events::blocked(&pool, withdraw_pid).await;
            let saved = tokio::spawn(async move { store.capture(write).await });
            super::events::blocked(&pool, write_pid).await;
            (saved, denied)
        };
        held.rollback().await.unwrap();
        let result = saved.await.unwrap();
        let current = denied.await.unwrap().unwrap();
        if write_first {
            assert!(result.unwrap().inserted);
        } else {
            assert_eq!(result, Err(OptionalEventError::Consent));
        }
        assert!(!current.analytics && !current.advertising);
        assert_eq!(
            PgOptionalEventStore::new(pool.clone(), clock)
                .capture(input(own, current.revision))
                .await,
            Err(OptionalEventError::Consent)
        );
        writes.close().await;
        withdrawals.close().await;
    }
    assert_eq!(rows(&pool).await, 1);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn blocked_insert_resamples_consent_session_and_clock_and_cancellation_never_commits() {
    let pool = auth_pool().await;
    sqlx::raw_sql("CREATE FUNCTION pause_optional_insert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(hashtextextended(current_schema(),0)); RETURN NEW; END $$; CREATE TRIGGER pause_optional_insert BEFORE INSERT ON optional_product_events FOR EACH ROW EXECUTE FUNCTION pause_optional_insert();").execute(&pool).await.unwrap();
    for change in 0..5 {
        let own = owner(super::personal::participant(&pool, 1000).await);
        let clock = Arc::new(Clock(AtomicI64::new(1100)));
        let first = PgConsentStore::new(pool.clone(), clock.clone())
            .refresh(
                own,
                None,
                &Grant {
                    analytics: true,
                    advertising: true,
                },
            )
            .await
            .unwrap();
        let (single, pid) = super::events::single_pool(&pool).await;
        let mut held = pool.acquire().await.unwrap();
        sqlx::query("SELECT pg_advisory_lock(hashtextextended(current_schema(),0))")
            .execute(&mut *held)
            .await
            .unwrap();
        let store = PgOptionalEventStore::new(single.clone(), clock.clone());
        let write = input(own, first.revision);
        let task = tokio::spawn(async move { store.capture(write).await });
        super::events::blocked(&pool, pid).await;
        if change >= 3 {
            if change == 3 {
                assert_eq!(
                    tokio::time::timeout(Duration::from_secs(3), task)
                        .await
                        .unwrap()
                        .unwrap(),
                    Err(OptionalEventError::Unavailable)
                );
                backend_stopped(&pool, pid, Duration::from_millis(500)).await;
            } else {
                task.abort();
                assert!(task.await.unwrap_err().is_cancelled());
                backend_stopped(&pool, pid, Duration::from_secs(3)).await;
            }
            sqlx::query("SELECT pg_advisory_unlock(hashtextextended(current_schema(),0))")
                .execute(&mut *held)
                .await
                .unwrap();
        } else {
            let (at, expected) = match change {
                0 => (2000, OptionalEventError::Consent),
                1 => (1099, OptionalEventError::Unavailable),
                _ => (1000 + SESSION_SECONDS, OptionalEventError::Unauthorized),
            };
            clock.0.store(at, Ordering::SeqCst);
            sqlx::query("SELECT pg_advisory_unlock(hashtextextended(current_schema(),0))")
                .execute(&mut *held)
                .await
                .unwrap();
            assert_eq!(task.await.unwrap(), Err(expected));
        }
        sqlx::query("SELECT 1").execute(&single).await.unwrap();
        assert_eq!(rows(&pool).await, 0, "change {change}");
        single.close().await;
    }
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn actual_logout_rotation_and_account_erasure_cascade_optional_raw_and_reject_old_authority()
{
    let pool = auth_pool().await;
    for change in 0..3 {
        let sessions = PgAuthStore::new(pool.clone());
        let token = SecretToken::generate().unwrap();
        let subject = Uuid::new_v4().to_string();
        sessions
            .login(super::rights::account_write(
                Provider::Google,
                &subject,
                &token,
                1000,
            ))
            .await
            .unwrap();
        let session = sessions.session(token.hash(), 1100).await.unwrap().unwrap();
        let own = ConsentSession {
            account: session.account.id,
            revision: session.id,
            hash: token.hash(),
        };
        let clock = Arc::new(Clock(AtomicI64::new(1100)));
        let consent = PgConsentStore::new(pool.clone(), clock.clone());
        let first = consent
            .refresh(
                own,
                None,
                &Grant {
                    analytics: true,
                    advertising: true,
                },
            )
            .await
            .unwrap();
        let store = PgOptionalEventStore::new(pool.clone(), clock);
        let write = input(own, first.revision);
        assert!(store.capture(write.clone()).await.unwrap().inserted);
        assert_eq!(rows(&pool).await, 1);
        match change {
            0 => sessions.logout(own.hash).await.unwrap(),
            1 => {
                let token = SecretToken::generate().unwrap();
                let mut login =
                    super::rights::account_write(Provider::Google, &subject, &token, 1101);
                login.previous_session = Some(own.hash);
                sessions.login(login).await.unwrap();
            }
            _ => {
                sessions
                    .erase_authorized(SessionAuthority {
                        account: own.account,
                        hash: own.hash,
                        now: 1100,
                    })
                    .await
                    .unwrap();
            }
        }
        assert_eq!(rows(&pool).await, 0);
        assert_eq!(
            store.capture(write).await,
            Err(OptionalEventError::Unauthorized)
        );
    }
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn regrant_preserves_an_existing_receipt_and_distinct_owner_or_payload_never_rewrites_it() {
    let pool = auth_pool().await;
    let own = owner(super::personal::participant(&pool, 1000).await);
    let other = owner(super::personal::participant(&pool, 1000).await);
    let clock = Arc::new(Clock(AtomicI64::new(1100)));
    let consent = PgConsentStore::new(pool.clone(), clock.clone());
    let first = consent
        .refresh(
            own,
            None,
            &Grant {
                analytics: true,
                advertising: true,
            },
        )
        .await
        .unwrap();
    let other_grant = consent
        .refresh(
            other,
            None,
            &Grant {
                analytics: true,
                advertising: true,
            },
        )
        .await
        .unwrap();
    let store = PgOptionalEventStore::new(pool.clone(), clock.clone());
    let original = input(own, first.revision);
    let receipt = store.capture(original.clone()).await.unwrap();
    let renewed = consent
        .refresh(
            own,
            Some(first.revision),
            &Grant {
                analytics: true,
                advertising: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        store.capture(original.clone()).await,
        Err(OptionalEventError::Consent)
    );
    let mut same = original.clone();
    same.expected_revision = renewed.revision;
    let repeat = store.capture(same.clone()).await.unwrap();
    assert!(!repeat.inserted);
    assert_eq!(repeat.event, receipt.event);
    assert_eq!(repeat.received_at, receipt.received_at);
    let proof: Uuid =
        sqlx::query_scalar("SELECT consent_revision FROM optional_product_events WHERE id=$1")
            .bind(receipt.event.id().uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(proof, first.revision);
    let mut foreign = same.clone();
    foreign.session = other;
    foreign.expected_revision = other_grant.revision;
    assert_eq!(
        store.capture(foreign).await,
        Err(OptionalEventError::Conflict)
    );
    let mut ads = input(own, renewed.revision);
    ads.draft = draft(Uuid::new_v4(), Uuid::new_v4(), true, 1100);
    assert_eq!(
        store.capture(ads).await.unwrap().event.purpose(),
        Purpose::Advertising
    );
    let denial = consent
        .withdraw(own, OptionalPurpose::Analytics)
        .await
        .unwrap();
    let mut ads = input(own, denial.revision);
    ads.draft = draft(Uuid::new_v4(), Uuid::new_v4(), true, 1100);
    assert!(store.capture(ads).await.unwrap().inserted);
    clock.0.store(1200, Ordering::SeqCst);
    let later = consent
        .refresh(
            own,
            Some(denial.revision),
            &Grant {
                analytics: true,
                advertising: true,
            },
        )
        .await
        .unwrap();
    same.expected_revision = later.revision;
    assert_eq!(
        store.capture(same).await,
        Err(OptionalEventError::Consent),
        "A fresh current grant cannot cover an older occurrence"
    );
    assert_eq!(rows(&pool).await, 3);
    let update =
        sqlx::query("UPDATE optional_product_events SET received_at=received_at+1 WHERE id=$1")
            .bind(receipt.event.id().uuid())
            .execute(&pool)
            .await
            .unwrap_err();
    assert_eq!(
        update.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    for (column, invalid) in [
        ("id", "'00000000-0000-0000-0000-000000000000'::uuid"),
        ("tab_key", "'00000000-0000-0000-0000-000000000000'::uuid"),
        (
            "consent_revision",
            "'00000000-0000-0000-0000-000000000000'::uuid",
        ),
        ("purpose", "'essential'"),
        ("locale", "'unknown'"),
        ("received_at", "occurred_at+2592000"),
        (
            "action",
            "'{\"type\":\"landing_view\",\"email\":\"forbidden\"}'::jsonb",
        ),
        ("action", "'{\"type\":\"unknown\"}'::jsonb"),
        (
            "action",
            "'{\"type\":\"ad_request\",\"placement\":\"menu\"}'::jsonb",
        ),
    ] {
        let columns = [
            "id",
            "owner_session_id",
            "tab_key",
            "consent_revision",
            "purpose",
            "occurred_at",
            "received_at",
            "locale",
            "action",
        ];
        let values = columns
            .iter()
            .map(|c| {
                if *c == column {
                    invalid
                } else if *c == "id" {
                    "gen_random_uuid()"
                } else {
                    c
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        let mut query =
            sqlx::QueryBuilder::<sqlx::Postgres>::new("INSERT INTO optional_product_events(");
        query
            .push(columns.join(","))
            .push(") SELECT ")
            .push(values)
            .push(" FROM optional_product_events WHERE id=")
            .push_bind(receipt.event.id().uuid());
        assert!(
            query.build().execute(&pool).await.is_err(),
            "invalid {column}"
        );
    }
    assert_eq!(rows(&pool).await, 3);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn four_real_blocked_writes_bound_capacity_and_cancelled_permits_are_reusable() {
    let pool = auth_pool().await;
    let own = owner(super::personal::participant(&pool, 1000).await);
    let clock = Arc::new(Clock(AtomicI64::new(1100)));
    let grant = PgConsentStore::new(pool.clone(), clock.clone())
        .refresh(
            own,
            None,
            &Grant {
                analytics: true,
                advertising: true,
            },
        )
        .await
        .unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let label = format!("optional_{}", Uuid::new_v4().simple());
    let app = label.clone();
    let writers=PgPoolOptions::new().min_connections(4).max_connections(4).after_connect(move |connection,_|{
        let schema=schema.clone();let app=app.clone();Box::pin(async move {
            sqlx::query("SELECT set_config('search_path',$1,false),set_config('application_name',$2,false)").bind(schema).bind(app).execute(connection).await?;Ok(())
        })
    }).connect(&env::var("DATABASE_URL").unwrap()).await.unwrap();
    let store = Arc::new(PgOptionalEventStore::new(writers.clone(), clock));
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(own.account)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let mut pending = vec![];
    for _ in 0..4 {
        let store = store.clone();
        let write = input(own, grant.revision);
        pending.push(tokio::spawn(async move { store.capture(write).await }));
    }
    tokio::time::timeout(Duration::from_secs(5),async {
        loop {if sqlx::query_scalar::<_,i64>("SELECT count(*) FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock'").bind(&label).fetch_one(&pool).await.unwrap()==4 {break;}tokio::time::sleep(Duration::from_millis(10)).await;}
    }).await.unwrap();
    assert_eq!(
        tokio::time::timeout(
            Duration::from_millis(500),
            store.capture(input(own, grant.revision))
        )
        .await
        .unwrap(),
        Err(OptionalEventError::Unavailable)
    );
    for task in pending {
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
    held.rollback().await.unwrap();
    assert!(
        store
            .capture(input(own, grant.revision))
            .await
            .unwrap()
            .inserted
    );
    assert_eq!(rows(&pool).await, 1);
    writers.close().await;
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL role administration in isolated test database"]
async fn limited_dml_role_can_capture_and_reap_but_cannot_mint_consent_mutate_events_or_migrate() {
    let pool = auth_pool().await;
    let own = owner(super::personal::participant(&pool, 1000).await);
    let clock = Arc::new(Clock(AtomicI64::new(1100)));
    let grant = PgConsentStore::new(pool.clone(), clock.clone())
        .refresh(
            own,
            None,
            &Grant {
                analytics: true,
                advertising: true,
            },
        )
        .await
        .unwrap();
    sqlx::query("DO $$ BEGIN CREATE ROLE liar_optional_test_dml NOLOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$").execute(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(isolated_schema_sql(
        "GRANT USAGE ON SCHEMA",
        &schema,
        "TO liar_optional_test_dml",
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql("GRANT SELECT ON auth_accounts,auth_sessions TO liar_optional_test_dml; GRANT UPDATE(id) ON auth_accounts,auth_sessions TO liar_optional_test_dml; GRANT SELECT ON session_consents TO liar_optional_test_dml; GRANT SELECT,INSERT,DELETE ON optional_product_events TO liar_optional_test_dml; GRANT UPDATE(id) ON optional_product_events TO liar_optional_test_dml; GRANT SELECT,DELETE ON product_events TO liar_optional_test_dml; GRANT UPDATE(id) ON product_events TO liar_optional_test_dml;").execute(&pool).await.unwrap();
    let dml = PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |connection, _| {
            let schema = schema.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(schema)
                    .execute(&mut *connection)
                    .await?;
                sqlx::query("SET ROLE liar_optional_test_dml")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    assert_eq!(
        PgConsentStore::new(dml.clone(), clock.clone())
            .read(own)
            .await
            .unwrap()
            .unwrap()
            .revision,
        grant.revision
    );
    let game = Uuid::new_v4();
    super::personal::seed_online(&pool, own.account, game, 1000).await;
    PgEventStore::new(pool.clone(), clock.clone())
        .capture(
            EventId::parse(&Uuid::new_v4().to_string()).unwrap(),
            EventSource::Online {
                account: EventId::parse(&own.account.to_string()).unwrap(),
                match_id: EventId::parse(&game.to_string()).unwrap(),
            },
        )
        .await
        .unwrap();
    let stored = PgOptionalEventStore::new(dml.clone(), clock)
        .capture(input(own, grant.revision))
        .await
        .unwrap();
    assert!(stored.inserted);
    for query in [
        "CREATE TABLE forbidden_optional(id integer)",
        "SELECT * FROM _sqlx_migrations",
        "UPDATE optional_product_events SET received_at=received_at+1",
        "UPDATE session_consents SET analytics=true",
        "UPDATE optional_product_events SET id=gen_random_uuid()",
        "UPDATE product_events SET id=gen_random_uuid()",
    ] {
        assert!(sqlx::raw_sql(query).execute(&dml).await.is_err(), "{query}");
    }
    assert_eq!(
        PgOptionalEventRetention::new(
            dml.clone(),
            Arc::new(Clock(AtomicI64::new(1100 + RAW_EVENT_SECONDS)))
        )
        .reap(Duration::from_secs(2))
        .await
        .unwrap()
        .deleted,
        1
    );
    assert_eq!(
        PgEventRetention::new(
            dml.clone(),
            Arc::new(Clock(AtomicI64::new(1100 + RAW_EVENT_SECONDS)))
        )
        .reap(Duration::from_secs(2))
        .await
        .unwrap()
        .deleted,
        1
    );
    assert_eq!(rows(&pool).await, 0);
    dml.close().await;
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn duplicate_receipt_time_is_monotonic_across_requests_and_invalid_authority_never_writes() {
    let pool = auth_pool().await;
    let own = owner(super::personal::participant(&pool, 1000).await);
    let clock = Arc::new(Clock(AtomicI64::new(1100)));
    let grant = PgConsentStore::new(pool.clone(), clock.clone())
        .refresh(
            own,
            None,
            &Grant {
                analytics: true,
                advertising: true,
            },
        )
        .await
        .unwrap();
    let store = PgOptionalEventStore::new(pool.clone(), clock.clone());
    let write = input(own, grant.revision);
    clock.0.store(1500, Ordering::SeqCst);
    assert_eq!(
        store.capture(write.clone()).await.unwrap().received_at,
        1500
    );
    clock.0.store(1499, Ordering::SeqCst);
    assert_eq!(
        store.capture(write.clone()).await,
        Err(OptionalEventError::Invalid)
    );
    clock.0.store(1501, Ordering::SeqCst);
    let repeat = store.capture(write.clone()).await.unwrap();
    assert!(!repeat.inserted);
    assert_eq!(repeat.received_at, 1500);
    let mut invalid = write.clone();
    invalid.expected_revision = Uuid::nil();
    assert_eq!(
        store.capture(invalid).await,
        Err(OptionalEventError::Invalid)
    );
    let mut invalid = write.clone();
    invalid.session.account = Uuid::nil();
    assert_eq!(
        store.capture(invalid).await,
        Err(OptionalEventError::Unauthorized)
    );
    let mut future = write.clone();
    future.draft = draft(Uuid::new_v4(), Uuid::new_v4(), false, 1502);
    assert_eq!(
        store.capture(future).await,
        Err(OptionalEventError::Invalid)
    );
    let closed = PgPoolOptions::new()
        .connect_lazy(&env::var("DATABASE_URL").unwrap())
        .unwrap();
    closed.close().await;
    assert_eq!(
        PgOptionalEventStore::new(closed, clock)
            .capture(write)
            .await,
        Err(OptionalEventError::Unavailable)
    );
    sqlx::query("UPDATE auth_sessions SET created_at=to_timestamp(-1),authenticated_at=to_timestamp(-1),expires_at=to_timestamp(2000) WHERE id=$1")
        .bind(own.revision).execute(&pool).await.unwrap();
    assert_eq!(
        store.capture(input(own, grant.revision)).await,
        Err(OptionalEventError::Unauthorized)
    );
    assert_eq!(rows(&pool).await, 1);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn pool_wait_is_subtracted_from_the_original_backend_deadline_and_clean_connections_recover()
{
    use futures_util::poll;
    let pool = auth_pool().await;
    let own = owner(super::personal::participant(&pool, 1000).await);
    let clock = Arc::new(Clock(AtomicI64::new(1100)));
    let grant = PgConsentStore::new(pool.clone(), clock.clone())
        .refresh(
            own,
            None,
            &Grant {
                analytics: true,
                advertising: true,
            },
        )
        .await
        .unwrap();
    sqlx::raw_sql("CREATE FUNCTION pause_optional_insert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(hashtextextended(current_schema(),0)); RETURN NEW; END $$; CREATE TRIGGER pause_optional_insert BEFORE INSERT ON optional_product_events FOR EACH ROW EXECUTE FUNCTION pause_optional_insert();").execute(&pool).await.unwrap();
    let (single, pid) = super::events::single_pool(&pool).await;
    let loaned = single.acquire().await.unwrap();
    let mut held = pool.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock(hashtextextended(current_schema(),0))")
        .execute(&mut *held)
        .await
        .unwrap();
    let store = Arc::new(PgOptionalEventStore::new(single.clone(), clock));
    let copy = store.clone();
    let write = input(own, grant.revision);
    let mut pending = Box::pin(async move { copy.capture(write).await });
    assert!(poll!(pending.as_mut()).is_pending());
    tokio::time::sleep(Duration::from_millis(750)).await;
    drop(loaned);
    let task = tokio::spawn(pending);
    super::events::blocked(&pool, pid).await;
    assert_eq!(task.await.unwrap(), Err(OptionalEventError::Unavailable));
    backend_stopped(&pool, pid, Duration::from_millis(500)).await;
    assert_eq!(rows(&pool).await, 0);
    sqlx::query("SELECT pg_advisory_unlock(hashtextextended(current_schema(),0))")
        .execute(&mut *held)
        .await
        .unwrap();
    let timeout: String = sqlx::query_scalar("SELECT current_setting('transaction_timeout')")
        .fetch_one(&single)
        .await
        .unwrap();
    assert_eq!(timeout, "0");
    assert!(
        store
            .capture(input(own, grant.revision))
            .await
            .unwrap()
            .inserted
    );
    let timeout: String = sqlx::query_scalar("SELECT current_setting('transaction_timeout')")
        .fetch_one(&single)
        .await
        .unwrap();
    assert_eq!(timeout, "0");
    sqlx::query("SELECT set_config('transaction_timeout','300ms',false)")
        .execute(&single)
        .await
        .unwrap();
    sqlx::query("SELECT pg_advisory_lock(hashtextextended(current_schema(),0))")
        .execute(&mut *held)
        .await
        .unwrap();
    let copy = store.clone();
    let write = input(own, grant.revision);
    let mut task = tokio::spawn(async move { copy.capture(write).await });
    let result = tokio::time::timeout(Duration::from_millis(1500), &mut task).await;
    if result.is_err() {
        task.abort();
        let _ = task.await;
        sqlx::query("SELECT pg_advisory_unlock(hashtextextended(current_schema(),0))")
            .execute(&mut *held)
            .await
            .unwrap();
        panic!(
            "A stricter existing transaction budget must not be extended to the request maximum"
        );
    }
    assert_eq!(
        result.unwrap().unwrap(),
        Err(OptionalEventError::Unavailable)
    );
    let renewed_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&single)
        .await
        .unwrap();
    assert_ne!(renewed_pid, pid);
    sqlx::query("SELECT pg_advisory_unlock(hashtextextended(current_schema(),0))")
        .execute(&mut *held)
        .await
        .unwrap();
    drop(held);
    single.close().await;
    close_auth_pool(pool).await;
}
