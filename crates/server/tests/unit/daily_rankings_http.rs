use super::*;

fn request() -> Request<Body> {
    Request::post("/api/v1/daily/rankings")
        .header(header::ORIGIN, "https://game.example")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"v":1,"date":"2026-10-07","seed_version":1}).to_string(),
        ))
        .unwrap()
}
#[tokio::test]
async fn public_headers_closed_input_and_date_bounds_reject_before_storage() {
    for mode in 0..13 {
        let f = Fixture::new(4);
        let mut input = request();
        match mode {
            0 => {
                input.headers_mut().remove(header::ORIGIN);
            }
            1 => {
                input
                    .headers_mut()
                    .insert(header::ORIGIN, "https://other.example".parse().unwrap());
            }
            2 => {
                input
                    .headers_mut()
                    .append(header::ORIGIN, "https://game.example".parse().unwrap());
            }
            3 => {
                input.headers_mut().remove(header::CONTENT_TYPE);
            }
            4 => {
                input
                    .headers_mut()
                    .append(header::CONTENT_TYPE, "application/json".parse().unwrap());
            }
            5 => *input.uri_mut() = "/api/v1/daily/rankings?offset=1".parse().unwrap(),
            6 => {
                *input.body_mut() = Body::from(
                    r#"{"v":1,"date":"2026-10-07","seed_version":1,"account_id":"extra"}"#,
                )
            }
            7 => *input.body_mut() = Body::from(r#"{"v":2,"date":"2026-10-07","seed_version":1}"#),
            8 => *input.body_mut() = Body::from(r#"{"v":1,"date":"2026-02-30","seed_version":1}"#),
            9 => *input.body_mut() = Body::from(r#"{"v":1,"date":"2026-10-07","seed_version":2}"#),
            10 => *input.body_mut() = Body::from("x".repeat(4097)),
            11 => {
                *input.body_mut() =
                    Body::from(r#"{"v":1,"v":1,"date":"2026-10-07","seed_version":1}"#)
            }
            12 => *input.body_mut() = Body::from("{}"),
            _ => unreachable!(),
        }
        assert_eq!(
            response(f.app, input).await.0,
            StatusCode::BAD_REQUEST,
            "mode {mode}"
        );
        assert_eq!(f.records.calls.load(Ordering::SeqCst), 0);
        assert_eq!(f.sessions.reads.load(Ordering::SeqCst), 0);
    }
}
#[tokio::test]
async fn global_rate_has_no_cookie_identity_and_clock_or_store_errors_are_not_empty_success() {
    let f = Fixture::new(4);
    for _ in 0..20 {
        assert_eq!(response(f.app.clone(), request()).await.0, StatusCode::OK);
    }
    let mut rotated = request();
    rotated
        .headers_mut()
        .insert(header::COOKIE, "untrusted=rotated".parse().unwrap());
    assert_eq!(
        response(f.app.clone(), rotated).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    f.clock.0.fetch_add(1, Ordering::SeqCst);
    assert_eq!(response(f.app.clone(), request()).await.0, StatusCode::OK);
    f.clock.0.store(NOW - 1, Ordering::SeqCst);
    assert_eq!(
        response(f.app.clone(), request()).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let f = Fixture::new(4);
    f.clock.0.store(0, Ordering::SeqCst);
    assert_eq!(
        response(f.app.clone(), request()).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    for mode in 0..3 {
        let f = Fixture::new(4);
        match mode {
            0 => *f.records.error.lock().unwrap() = Some(DailyStoreError::Unavailable),
            1 => *f.records.after.lock().unwrap() = Some((f.clock.clone(), NOW - 1)),
            2 => {
                *f.records.accepted.lock().unwrap() =
                    Some(PublishedDaily::new("2026-10-06", 1, RulesSnapshot::bundled()).unwrap())
            }
            _ => unreachable!(),
        }
        assert_eq!(
            response(f.app.clone(), request()).await.0,
            StatusCode::SERVICE_UNAVAILABLE,
            "mode {mode}"
        );
    }
}
#[tokio::test]
async fn two_public_read_permits_and_the_entire_deadline_bound_slow_storage() {
    let f = Fixture::new(4);
    f.records.hold.store(true, Ordering::SeqCst);
    let first = tokio::spawn(response(f.app.clone(), request()));
    f.records.entered.notified().await;
    let second = tokio::spawn(response(f.app.clone(), request()));
    f.records.entered.notified().await;
    assert_eq!(
        response(f.app.clone(), request()).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), first)
            .await
            .unwrap()
            .unwrap()
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(second.await.unwrap().0, StatusCode::SERVICE_UNAVAILABLE);
    f.records.hold.store(false, Ordering::SeqCst);
    assert_eq!(response(f.app.clone(), request()).await.0, StatusCode::OK);
}
struct BlockingVerifier {
    entered: Notify,
    release: Mutex<bool>,
    ready: std::sync::Condvar,
}
impl BlockingVerifier {
    fn release(&self) {
        *self.release.lock().unwrap() = true;
        self.ready.notify_all();
    }
}
struct Release(Arc<BlockingVerifier>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
}
impl ReplayVerifier for BlockingVerifier {
    fn verify(
        &self,
        definition: &PublishedDaily,
        source: &str,
    ) -> Result<ReplayEvidence, ReplayError> {
        self.entered.notify_one();
        let mut release = self.release.lock().unwrap();
        while !*release {
            release = self.ready.wait(release).unwrap();
        }
        NativeReplayVerifier.verify(definition, source)
    }
}
#[tokio::test]
async fn public_decode_shares_the_two_live_native_worker_budget_with_authenticated_submissions() {
    let gate = Arc::new(BlockingVerifier {
        entered: Notify::new(),
        release: Mutex::new(false),
        ready: std::sync::Condvar::new(),
    });
    let _release = Release(gate.clone());
    let f = Fixture::with_verifier(4, gate.clone());
    let first = tokio::spawn(response(
        f.app.clone(),
        f.request("submit", f.body("submit")),
    ));
    gate.entered.notified().await;
    let second = tokio::spawn(response(
        f.app.clone(),
        f.request("submit", f.body("submit")),
    ));
    gate.entered.notified().await;
    let blocked = response(f.app.clone(), request()).await.0;
    gate.release();
    assert_eq!(blocked, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(first.await.unwrap().0, StatusCode::OK);
    assert_eq!(second.await.unwrap().0, StatusCode::OK);
    assert_eq!(response(f.app.clone(), request()).await.0, StatusCode::OK);
}
