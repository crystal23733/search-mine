use super::*;

struct TimingHttp {
    pool: sqlx::PgPool,
    clock: Arc<Clock>,
    tickets: Arc<DailyTicketRegistry>,
    store: PgAuthStore,
    security: Arc<BrowserSecurity>,
    app: axum::Router,
}
impl TimingHttp {
    async fn new() -> Self {
        let pool = auth_pool().await;
        let clock = Arc::new(Clock(AtomicI64::new(NOW)));
        let authorities = AuthorityRegistry::new(1).unwrap();
        let tickets = Arc::new(DailyTicketRegistry::new(1, authorities, clock.clone()).unwrap());
        let store = PgAuthStore::new(pool.clone()).with_invalidations(tickets.clone());
        let security = Arc::new(BrowserSecurity::new("https://game.example", [122; 32]).unwrap());
        let sessions = Arc::new(PgSessionReader::new(store.clone(), clock.clone()));
        let app = daily_router(
            Arc::new(PgDailyRepository::new(pool.clone(), clock.clone())),
            BoundedReplayVerifier::new(Arc::new(NativeReplayVerifier)),
            sessions,
            security.clone(),
            DailyAuthentication::new(tickets.clone(), clock.clone()),
            4,
        )
        .unwrap();
        Self {
            pool,
            clock,
            tickets,
            store,
            security,
            app,
        }
    }
    fn set(&self, seconds: i64) {
        self.clock.0.store(NOW + seconds, Ordering::SeqCst);
    }
    async fn call(&self, client: &Client, path: &str, input: Value) -> (StatusCode, Value) {
        response(
            self.app.clone(),
            request(
                client,
                &self.security,
                &format!("/api/v1/daily/{path}"),
                input,
                self.clock.now(),
            ),
        )
        .await
    }
    fn identity(client: &Client) -> Value {
        json!({"v":1,"account_id":client.account.to_string(),"session_revision":client.revision.to_string()})
    }
    async fn start(&self, client: &Client, attempt: Uuid) -> Value {
        let mut input = Self::identity(client);
        input["attempt_id"] = json!(attempt.to_string());
        let (status, reply) = self.call(client, "start", input).await;
        assert_eq!(status, StatusCode::OK, "{reply}");
        reply
    }
    async fn heartbeat(
        &self,
        client: &Client,
        ticket: &Value,
        sequence: u32,
    ) -> (StatusCode, Value) {
        let mut input = Self::identity(client);
        input["ticket_id"] = ticket.clone();
        input["sequence"] = json!(sequence);
        self.call(client, "heartbeat", input).await
    }
    fn finish(client: &Client, attempt: Uuid, ticket: Value) -> Value {
        let mut input = Self::identity(client);
        input["attempt_id"] = json!(attempt.to_string());
        input["ticket_id"] = ticket;
        input["replay"] = replay();
        input
    }
}

#[tokio::test]
#[ignore = "requires actual PostgreSQL; executed in database CI"]
async fn timing_routes_native_pg_original_time_retry_history_and_delete_use_the_same_authority() {
    let f = TimingHttp::new().await;
    let client = participant(&f.store).await;
    let attempt = Uuid::new_v4();
    let (started, concurrent_start) =
        tokio::join!(f.start(&client, attempt), f.start(&client, attempt));
    assert_eq!(started, concurrent_start);
    assert_eq!(started["metadata"], replay()["metadata"]);
    f.set(1);
    assert_eq!(f.start(&client, attempt).await, started);
    f.set(4);
    let first = f.heartbeat(&client, &started["ticket_id"], 1).await;
    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(first.1["ack"], json!({"sequence":1,"elapsed_ms":4000}));
    assert_eq!(
        f.heartbeat(&client, &started["ticket_id"], 3).await.0,
        StatusCode::CONFLICT
    );
    f.set(5);
    assert_eq!(
        f.heartbeat(&client, &started["ticket_id"], 1).await.1,
        first.1
    );
    f.set(12);
    let input = TimingHttp::finish(&client, attempt, started["ticket_id"].clone());
    let ((status, saved), concurrent_finish) = tokio::join!(
        f.call(&client, "submit-timed", input.clone()),
        f.call(&client, "submit-timed", input.clone())
    );
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(concurrent_finish, (status, saved.clone()));
    assert_eq!(saved["trust"], "verified_time");
    assert_eq!(saved["receipt"]["verified_elapsed_ms"], 9000);
    f.set(13);
    assert_eq!(
        f.call(&client, "submit-timed", input.clone()).await.1,
        saved
    );
    let mut offline = input.clone();
    offline["ticket_id"] = Value::Null;
    assert_eq!(
        f.call(&client, "submit-timed", offline.clone()).await.1,
        saved
    );
    offline["attempt_id"] = json!(Uuid::new_v4().to_string());
    let practice = f.call(&client, "submit-timed", offline).await;
    assert_eq!(practice.0, StatusCode::OK);
    assert_eq!(practice.1["disposition"], "practice");
    assert_eq!(practice.1["receipt"], saved["receipt"]);
    let reader = PgMeReader::new(f.pool.clone(), f.clock.clone());
    let page = crate::personal::daily_history(
        &reader,
        MeSession {
            account: client.account,
            revision: client.revision,
            hash: client.token.hash(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(&page.items[0]).unwrap()["entry"]["verified_elapsed_ms"],
        9000
    );
    let barrier = f.tickets.account(client.account);
    assert_eq!(
        f.call(&client, "submit-timed", input.clone()).await.0,
        StatusCode::UNAUTHORIZED
    );
    f.store
        .erase_authorized(SessionAuthority {
            account: client.account,
            hash: client.token.hash(),
            now: f.clock.now(),
        })
        .await
        .unwrap();
    drop(barrier);
    assert_eq!(
        f.call(&client, "submit-timed", input).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM daily_completions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(f.pool).await;
}

#[tokio::test]
#[ignore = "requires actual PostgreSQL; executed in database CI"]
async fn missing_gap_ttl_restart_and_too_fast_tickets_only_store_completion() {
    for mode in 0..5 {
        let mut f = TimingHttp::new().await;
        let client = participant(&f.store).await;
        let attempt = Uuid::new_v4();
        let started = f.start(&client, attempt).await;
        let ticket = match mode {
            0 => Value::Null,
            1 => {
                f.set(45);
                assert!(f.heartbeat(&client, &started["ticket_id"], 1).await.1["ack"].is_null());
                started["ticket_id"].clone()
            }
            2 => {
                f.set(started["ttl_ms"].as_i64().unwrap() / 1000);
                started["ticket_id"].clone()
            }
            3 => {
                // Reconstruct the production adapters with an empty in-memory registry.
                f.tickets = Arc::new(
                    DailyTicketRegistry::new(
                        1,
                        AuthorityRegistry::new(1).unwrap(),
                        f.clock.clone(),
                    )
                    .unwrap(),
                );
                f.store = PgAuthStore::new(f.pool.clone()).with_invalidations(f.tickets.clone());
                f.app = daily_router(
                    Arc::new(PgDailyRepository::new(f.pool.clone(), f.clock.clone())),
                    BoundedReplayVerifier::new(Arc::new(NativeReplayVerifier)),
                    Arc::new(PgSessionReader::new(f.store.clone(), f.clock.clone())),
                    f.security.clone(),
                    DailyAuthentication::new(f.tickets.clone(), f.clock.clone()),
                    4,
                )
                .unwrap();
                f.set(12);
                started["ticket_id"].clone()
            }
            _ => {
                f.set(1);
                started["ticket_id"].clone()
            }
        };
        let (status, saved) = f
            .call(
                &client,
                "submit-timed",
                TimingHttp::finish(&client, attempt, ticket),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "mode {mode}: {saved}");
        assert_eq!(saved["trust"], "verified_completion");
        assert!(saved["receipt"]["verified_elapsed_ms"].is_null());
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM daily_completions WHERE verified_elapsed_ms IS NULL"
            )
            .fetch_one(&f.pool)
            .await
            .unwrap(),
            1
        );
        close_auth_pool(f.pool).await;
    }
}

#[tokio::test]
#[ignore = "requires actual PostgreSQL; executed in database CI"]
async fn daily_http_reaps_expired_last_lease_before_another_account_reads_or_binds() {
    let f = TimingHttp::new().await;
    let first = participant(&f.store).await;
    let second = participant(&f.store).await;
    let started = f.start(&first, Uuid::new_v4()).await;
    let mut input = body(&second, "own");
    assert_eq!(
        f.call(&second, "own", input.clone()).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    f.set(started["ttl_ms"].as_i64().unwrap() / 1000);
    let (status, reply) = f.call(&second, "own", input.clone()).await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    input["account_id"] = json!(first.account.to_string());
    assert_eq!(
        f.call(&second, "own", input).await.0,
        StatusCode::UNAUTHORIZED
    );
    close_auth_pool(f.pool).await;
}

#[tokio::test]
#[ignore = "requires actual PostgreSQL; executed in database CI"]
async fn wrong_attempt_or_accepted_metadata_never_falls_back_to_plain_completion() {
    for wrong_metadata in [false, true] {
        let f = TimingHttp::new().await;
        let client = participant(&f.store).await;
        if wrong_metadata {
            f.set(86400);
        }
        let attempt = Uuid::new_v4();
        let started = f.start(&client, attempt).await;
        let input = TimingHttp::finish(
            &client,
            if wrong_metadata {
                attempt
            } else {
                Uuid::new_v4()
            },
            started["ticket_id"].clone(),
        );
        let (status, _) = f.call(&client, "submit-timed", input).await;
        assert_eq!(
            status,
            if wrong_metadata {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::CONFLICT
            }
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM daily_completions")
                .fetch_one(&f.pool)
                .await
                .unwrap(),
            0
        );
        close_auth_pool(f.pool).await;
    }
}
