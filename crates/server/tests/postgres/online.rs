use super::rights::account_write;
use super::*;
use liar_server::online::{
    AuthorityRegistry, FinishedMatch, PgResultRepository, PlayerResult, ResultRepository,
    SaveResult,
};

struct ResultClock;
impl AuthClock for ResultClock {
    fn now(&self) -> i64 {
        19000
    }
}
pub(super) fn finished(accounts: [Option<Uuid>; 2]) -> FinishedMatch {
    use liar_protocol::game::{Outcome, PublicEndReason};
    FinishedMatch {
        id: Uuid::new_v4(),
        rules_hash: liar_core::rules::RulesSnapshot::bundled().hash.clone(),
        seed: [46; 8],
        ended_ms: 243000,
        reason: PublicEndReason::Timeout,
        players: accounts.map(|account| PlayerResult {
            account,
            outcome: Outcome::Draw,
            opened_safe: 4,
            mistakes: 0,
            accusations: 0,
            correct_accusations: 0,
        }),
    }
}
async fn participant(store: &PgAuthStore) -> Uuid {
    let token = SecretToken::generate().unwrap();
    store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            18000,
        ))
        .await
        .unwrap()
        .account
        .id
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn runtime_session_reader_and_logout_share_connection_invalidation() {
    use liar_server::online::{PgSessionReader, SessionReader};
    let pool = auth_pool().await;
    let registry = AuthorityRegistry::new(2).unwrap();
    let runtime = RuntimeAuthConfig {
        security: BrowserSecurity::new("https://game.example", [5; 32]).unwrap(),
        vault: AeadVault::new(1, vec![(1, [6; 32])]).unwrap(),
        digests: DigestKeys::new(1, vec![(1, [7; 32])]).unwrap(),
        providers: vec![],
        notification_audience: None,
    }
    .initialize_with_invalidations(pool.clone(), registry.clone())
    .unwrap();
    let now = SystemAuthClock.now();
    let token = SecretToken::generate().unwrap();
    let account = runtime
        .store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            now,
        ))
        .await
        .unwrap()
        .account
        .id;
    let reader = PgSessionReader::new(runtime.store.clone(), Arc::new(SystemAuthClock));
    let session = reader
        .read(token.hash())
        .await
        .unwrap()
        .expect("Stored service session must be available");
    let lease = registry
        .bind(
            registry.generation().unwrap(),
            account,
            token.hash(),
            session.expires_at,
            now,
        )
        .unwrap();
    runtime.store.logout(token.hash()).await.unwrap();
    assert!(reader.read(token.hash()).await.unwrap().is_none());
    assert!(registry.with_authority(&lease, now, || ()).is_err());
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn authenticated_lobby_http_reads_real_nickname_and_logout_cancels_an_initial_assignment() {
    use liar_core::rules::RulesSnapshot;
    use liar_server::{lobby::*, online::*};
    use std::sync::atomic::{AtomicU64, Ordering};
    struct GameClock(AtomicU64);
    impl MatchClock for GameClock {
        fn now_ms(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }
    let pool = auth_pool().await;
    let sockets = AuthorityRegistry::new(4).unwrap();
    let lobby = AuthorityRegistry::new(4).unwrap();
    let runtime = RuntimeAuthConfig {
        security: BrowserSecurity::new("https://game.example", [5; 32]).unwrap(),
        vault: AeadVault::new(1, vec![(1, [6; 32])]).unwrap(),
        digests: DigestKeys::new(1, vec![(1, [7; 32])]).unwrap(),
        providers: vec![],
        notification_audience: None,
    }
    .initialize_with_invalidations(
        pool.clone(),
        Arc::new(CombinedSessionInvalidator::new(
            sockets.clone(),
            lobby.clone(),
        )),
    )
    .unwrap();
    let clock = Arc::new(GameClock(AtomicU64::new(0)));
    let auth_clock = Arc::new(SystemAuthClock);
    let registry = MatchRegistry::new(
        MatchLimits {
            matches: 1,
            mailbox: 16,
            outgoing: 16,
            proof_workers: 1,
        },
        clock.clone(),
        auth_clock.clone(),
        sockets,
        Arc::new(PgResultRepository::new(pool.clone(), auth_clock.clone())),
    )
    .unwrap();
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let service = LobbyService::new(
        LobbyLimits {
            capacity: 4,
            workers: 1,
        },
        Arc::new(
            BoardPool::start(
                RulesSnapshot::from_rules(rules).unwrap(),
                2,
                1,
                Arc::new(OsSeedSource),
            )
            .unwrap(),
        ),
        Arc::new(CoreMatchPreparer),
        Arc::new(OsRoomCodeSource),
        registry.clone(),
        BotExecutor::new(1, Arc::new(CoreBotFactory)).unwrap(),
        LobbyAuthentication {
            authorities: lobby,
            clock: auth_clock,
        },
    )
    .unwrap();
    let router = lobby_router(
        service,
        Arc::new(PgSessionReader::new(
            runtime.store.clone(),
            Arc::new(SystemAuthClock),
        )),
        runtime.security.clone(),
        2,
    )
    .unwrap();
    let a = SecretToken::generate().unwrap();
    let b = SecretToken::generate().unwrap();
    let browser = SecretToken::generate().unwrap();
    let now = SystemAuthClock.now();
    let account_a = runtime
        .store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &a,
            now,
        ))
        .await
        .unwrap()
        .account
        .id;
    let account_b = runtime
        .store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &b,
            now,
        ))
        .await
        .unwrap()
        .account
        .id;
    let request = |token: &SecretToken, command: serde_json::Value| {
        Request::builder()
            .method("POST")
            .uri("/api/v1/lobby")
            .header("origin", "https://game.example")
            .header("content-type", "application/json")
            .header(
                "cookie",
                format!(
                    "{BROWSER_COOKIE}={}; {SESSION_COOKIE}={}",
                    browser.expose().as_str(),
                    token.expose().as_str()
                ),
            )
            .header(
                "x-liar-csrf",
                runtime
                    .security
                    .csrf(&browser, Some(token), SystemAuthClock.now())
                    .unwrap(),
            )
            .body(Body::from(
                serde_json::json!({"v":1,"command":command}).to_string(),
            ))
            .unwrap()
    };
    let join = serde_json::json!({"type":"queue_join","difficulty":"normal"});
    assert_eq!(
        router
            .clone()
            .oneshot(request(&a, join.clone()))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(
        runtime
            .store
            .nickname(account_a, Nickname::parse("Lobby探偵A").unwrap())
            .await
            .unwrap()
    );
    assert!(
        runtime
            .store
            .nickname(account_b, Nickname::parse("Lobby探偵B").unwrap())
            .await
            .unwrap()
    );
    for token in [&a, &b] {
        let response = router
            .clone()
            .oneshot(request(token, join.clone()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    tokio::time::timeout(Duration::from_secs(3), async {
        while registry.for_account(account_a).is_err() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let id = registry.for_account(account_a).unwrap().id();
    assert_eq!(registry.for_account(account_b).unwrap().id(), id);
    for (seat, token) in [&a, &b].into_iter().enumerate() {
        let response = router
            .clone()
            .oneshot(request(token, serde_json::json!({"type":"status"})))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(value["state"]["match_id"], id.to_string());
        assert_eq!(value["state"]["own_seat"], seat);
        assert_eq!(value["state"]["opponent"], "human");
        assert!(value["state"].get("account").is_none());
    }
    runtime.store.logout(a.hash()).await.unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(request(&a, join))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let reason: Option<String> =
                sqlx::query_scalar("SELECT reason FROM online_match_results WHERE id=$1")
                    .bind(id)
                    .fetch_optional(&pool)
                    .await
                    .unwrap();
            if let Some(reason) = reason {
                assert_eq!(reason, "cancelled");
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(registry.for_account(account_a).is_err());
    assert!(registry.for_account(account_b).is_err());
    let response = router
        .oneshot(request(&b, serde_json::json!({"type":"status"})))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(value["state"]["type"], "idle");
    let (elapsed,players):(i64,i64)=sqlx::query_as("SELECT end_elapsed_ms,(SELECT count(*) FROM online_match_players WHERE match_id=$1) FROM online_match_results WHERE id=$1").bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!((elapsed, players), (0, 2));
    clock.0.store(30000, Ordering::SeqCst);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn persistent_logout_revocation_and_erasure_barrier_cover_lobby_and_socket_before_commit() {
    let pool = auth_pool().await;
    for kind in 0..3 {
        let sockets = AuthorityRegistry::new(2).unwrap();
        let lobby = AuthorityRegistry::new(2).unwrap();
        let store = PgAuthStore::new(pool.clone()).with_invalidations(Arc::new(
            CombinedSessionInvalidator::new(sockets.clone(), lobby.clone()),
        ));
        let subject = Uuid::new_v4().to_string();
        let token = SecretToken::generate().unwrap();
        let account = store
            .login(account_write(Provider::Google, &subject, &token, 18000))
            .await
            .unwrap()
            .account
            .id;
        let other_token = SecretToken::generate().unwrap();
        assert_eq!(
            store
                .login(account_write(
                    Provider::Google,
                    &subject,
                    &other_token,
                    18000
                ))
                .await
                .unwrap()
                .account
                .id,
            account
        );
        let session = store.session(token.hash(), 18001).await.unwrap().unwrap();
        let socket = sockets
            .bind(
                sockets.generation().unwrap(),
                account,
                token.hash(),
                session.expires_at,
                18001,
            )
            .unwrap();
        let lease = lobby
            .bind_shared(
                lobby.generation().unwrap(),
                account,
                token.hash(),
                session.expires_at,
                18001,
            )
            .unwrap();
        let unaffected = lobby
            .bind_shared(
                lobby.generation().unwrap(),
                Uuid::new_v4(),
                [7; 32],
                session.expires_at,
                18001,
            )
            .unwrap();
        let before = [sockets.generation().unwrap(), lobby.generation().unwrap()];
        let mut socket_revoked = socket.revoked();
        let mut lobby_revoked = lease.revoked();
        let mut held = pool.begin().await.unwrap();
        if kind == 2 {
            // Erasure can authorize normally, then blocks on this cascading identity delete.
            sqlx::query("SELECT id FROM auth_identities WHERE account_id=$1 FOR UPDATE")
                .bind(account)
                .fetch_one(&mut *held)
                .await
                .unwrap();
        } else {
            sqlx::query("SELECT id FROM auth_sessions WHERE token_hash=$1 FOR UPDATE")
                .bind(token.hash().as_slice())
                .fetch_one(&mut *held)
                .await
                .unwrap();
        }
        let mutating = store.clone();
        let hash = token.hash();
        let mutation = tokio::spawn(async move {
            match kind {
                0 => mutating.logout(hash).await,
                1 => mutating.revoke_account(account).await,
                _ => mutating
                    .erase_authorized(SessionAuthority {
                        account,
                        hash,
                        now: 18001,
                    })
                    .await
                    .map(|_| ()),
            }
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while !*socket_revoked.borrow() {
                socket_revoked.changed().await.unwrap();
            }
            while !*lobby_revoked.borrow() {
                lobby_revoked.changed().await.unwrap();
            }
        })
        .await
        .expect("both in-memory authorities must be retired before the blocked database commit");
        assert!(!mutation.is_finished());
        assert!(
            store.session(token.hash(), 18001).await.unwrap().is_some(),
            "the database has not committed deletion yet"
        );
        assert!(lobby.with_authorities(&[&lease], 18001, || ()).is_err());
        assert_eq!(lobby.with_authority(&unaffected, 18001, || 7), Ok(7));
        let during = [sockets.generation().unwrap(), lobby.generation().unwrap()];
        for (index, registry) in [&sockets, &lobby].into_iter().enumerate() {
            for generation in [before[index], during[index]] {
                assert!(
                    registry
                        .bind_shared(generation, account, hash, session.expires_at, 18001)
                        .is_err()
                );
            }
        }
        held.rollback().await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), mutation)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(store.session(hash, 18001).await.unwrap().is_none());
        assert_eq!(
            store
                .session(other_token.hash(), 18001)
                .await
                .unwrap()
                .is_some(),
            kind == 0
        );
        for (index, registry) in [&sockets, &lobby].into_iter().enumerate() {
            assert!(
                registry
                    .bind_shared(during[index], account, hash, session.expires_at, 18001)
                    .is_err()
            );
        }
        let account_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM auth_accounts WHERE id=$1)")
                .bind(account)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(account_exists, kind != 2);
    }
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn result_waiting_on_account_lock_skips_a_concurrently_deleted_participant() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let account = participant(&store).await;
    let other = participant(&store).await;
    let result = finished([Some(account), Some(other)]);
    let id = result.id;
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let save_pool = PgPoolOptions::new()
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
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&save_pool)
        .await
        .unwrap();
    let mut deletion = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(account)
        .fetch_one(&mut *deletion)
        .await
        .unwrap();
    let repository = PgResultRepository::new(save_pool.clone(), Arc::new(ResultClock));
    let saving = tokio::spawn(async move { repository.save(result).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let blocked: bool = sqlx::query_scalar("SELECT cardinality(pg_blocking_pids($1)) > 0")
                .bind(pid)
                .fetch_one(&pool)
                .await
                .unwrap();
            if blocked {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Result write must wait for the held account lock");
    // Exercise the account-row deletion effect while owning the production deletion lock.
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(account)
        .execute(&mut *deletion)
        .await
        .unwrap();
    deletion.commit().await.unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), saving)
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
        SaveResult::Saved
    );
    let remaining: Vec<Uuid> =
        sqlx::query_scalar("SELECT account_id FROM online_match_players WHERE match_id=$1")
            .bind(id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, vec![other]);
    save_pool.close().await;
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn final_result_saves_one_parent_and_both_players_atomically_under_concurrent_retry() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let accounts = [
        Some(participant(&store).await),
        Some(participant(&store).await),
    ];
    let result = finished(accounts);
    let repository = PgResultRepository::new(pool.clone(), Arc::new(ResultClock));
    let (a, b) = tokio::join!(
        repository.save(result.clone()),
        repository.save(result.clone())
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert!(matches!(
        (a, b),
        (SaveResult::Saved, SaveResult::Duplicate) | (SaveResult::Duplicate, SaveResult::Saved)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM online_match_results WHERE id=$1")
            .bind(result.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM online_match_players WHERE match_id=$1")
            .bind(result.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    let mut conflict = result.clone();
    conflict.ended_ms += 1;
    assert!(repository.save(conflict).await.is_err());
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn deleting_a_participant_and_retrying_cannot_recreate_their_record_or_nickname() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let token = SecretToken::generate().unwrap();
    let account = store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            18000,
        ))
        .await
        .unwrap()
        .account
        .id;
    let other = participant(&store).await;
    let result = finished([Some(account), Some(other)]);
    let repository = PgResultRepository::new(pool.clone(), Arc::new(ResultClock));
    assert_eq!(
        repository.save(result.clone()).await.unwrap(),
        SaveResult::Saved
    );
    store
        .erase_authorized(SessionAuthority {
            account,
            hash: token.hash(),
            now: 18001,
        })
        .await
        .unwrap();
    assert_eq!(
        repository.save(result.clone()).await.unwrap(),
        SaveResult::Duplicate
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM online_match_players WHERE match_id=$1 AND account_id=$2"
        )
        .bind(result.id)
        .bind(account)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    let fresh_result = finished([Some(account), Some(other)]);
    assert_eq!(
        repository.save(fresh_result.clone()).await.unwrap(),
        SaveResult::Saved
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM online_match_players WHERE match_id=$1")
            .bind(fresh_result.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let columns: Vec<String> = sqlx::query_scalar("SELECT column_name FROM information_schema.columns WHERE table_schema=current_schema() AND table_name IN ('online_match_players','online_match_results')").fetch_all(&pool).await.unwrap();
    for forbidden in [
        "nickname",
        "token_hash",
        "subject_digest",
        "email",
        "password",
    ] {
        assert!(!columns.iter().any(|column| column == forbidden));
    }
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn second_participant_sql_failure_rolls_back_the_parent_and_first_participant() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let accounts = [
        Some(participant(&store).await),
        Some(participant(&store).await),
    ];
    let repository = PgResultRepository::new(pool.clone(), Arc::new(ResultClock));
    assert_eq!(
        repository.save(finished(accounts)).await.unwrap(),
        SaveResult::Saved
    );
    sqlx::query("ALTER TABLE online_match_players ADD CONSTRAINT fixture_fail_second CHECK (seat=0) NOT VALID").execute(&pool).await.unwrap();
    let failed = finished(accounts);
    assert!(repository.save(failed.clone()).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM online_match_results WHERE id=$1")
            .bind(failed.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM online_match_players WHERE match_id=$1")
            .bind(failed.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn logout_revokes_memory_authority_before_its_blocked_sql_write_can_commit() {
    let pool = auth_pool().await;
    let registry = AuthorityRegistry::new(2).unwrap();
    let store = PgAuthStore::new(pool.clone()).with_invalidations(registry.clone());
    let token = SecretToken::generate().unwrap();
    let record = store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            16000,
        ))
        .await
        .unwrap();
    let lease = registry
        .bind(
            registry.generation().unwrap(),
            record.account.id,
            token.hash(),
            20000,
            16000,
        )
        .unwrap();
    let mut revoked = lease.revoked();
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_sessions WHERE token_hash=$1 FOR UPDATE")
        .bind(token.hash().as_slice())
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let hash = token.hash();
    let logout = tokio::spawn(async move { store.logout(hash).await });
    tokio::time::timeout(Duration::from_secs(3), revoked.changed())
        .await
        .expect("memory authority must be revoked before the blocked SQL commit")
        .unwrap();
    assert!(*revoked.borrow());
    assert!(
        registry
            .with_authority(&lease, 16000, || "must not apply")
            .is_err()
    );
    let during = registry.generation().unwrap();
    assert!(
        registry
            .bind(during, record.account.id, hash, 20000, 16000)
            .is_err()
    );
    blocker.rollback().await.unwrap();
    logout.await.unwrap().unwrap();
    assert!(
        PgAuthStore::new(pool.clone())
            .session(hash, 16000)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        registry
            .bind(during, record.account.id, hash, 20000, 16000)
            .is_err()
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn account_erasure_revokes_an_existing_connection_authority() {
    let pool = auth_pool().await;
    let registry = AuthorityRegistry::new(1).unwrap();
    let store = PgAuthStore::new(pool.clone()).with_invalidations(registry.clone());
    let token = SecretToken::generate().unwrap();
    let record = store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            17000,
        ))
        .await
        .unwrap();
    let lease = registry
        .bind(
            registry.generation().unwrap(),
            record.account.id,
            token.hash(),
            20000,
            17000,
        )
        .unwrap();
    store
        .erase_authorized(SessionAuthority {
            account: record.account.id,
            hash: token.hash(),
            now: 17001,
        })
        .await
        .unwrap();
    assert!(
        registry
            .with_authority(&lease, 17001, || "must not apply")
            .is_err()
    );
    assert!(*lease.revoked().borrow());
    close_auth_pool(pool).await;
}
