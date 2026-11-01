use super::*;
use liar_server::consent::*;
use std::sync::atomic::{AtomicI64, Ordering};

struct Clock(AtomicI64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
fn session(value: liar_server::privacy::ExportSession) -> ConsentSession {
    ConsentSession {
        account: value.account,
        revision: value.revision,
        hash: value.hash,
    }
}
struct Verifier {
    analytics: bool,
    advertising: bool,
    expiry: i64,
}
impl ConsentVerifier for Verifier {
    async fn verify(&self, subject: Uuid, _: i64) -> Result<VerifiedConsent, ConsentError> {
        Ok(VerifiedConsent {
            subject,
            analytics: self.analytics,
            advertising: self.advertising,
            expires_at: self.expiry,
        })
    }
}
fn grant() -> Verifier {
    Verifier {
        analytics: true,
        advertising: true,
        expiry: 2000,
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn current_consent_is_session_bound_separate_and_cascades_on_logout_and_deletion() {
    let pool = auth_pool().await;
    let own = super::privacy::participant(&pool, 1000).await;
    let other = super::privacy::participant(&pool, 1000).await;
    let clock = Arc::new(Clock(AtomicI64::new(1100)));
    let store = PgConsentStore::new(pool.clone(), clock.clone());
    assert_eq!(store.read(session(own)).await, Ok(None));
    assert_eq!(
        store
            .refresh(session(own), None, &DisabledConsentVerifier)
            .await,
        Err(ConsentError::Unavailable)
    );
    let first = store.refresh(session(own), None, &grant()).await.unwrap();
    assert_eq!(store.read(session(own)).await, Ok(Some(first)));
    assert_eq!(store.read(session(other)).await, Ok(None));
    let mut foreign = session(own);
    foreign.revision = other.revision;
    assert_eq!(store.read(foreign).await, Err(ConsentError::Unauthorized));
    foreign = session(own);
    foreign.hash = other.hash;
    assert_eq!(store.read(foreign).await, Err(ConsentError::Unauthorized));
    let withdrawn = store
        .withdraw(session(own), OptionalPurpose::Analytics)
        .await
        .unwrap();
    assert!(!withdrawn.analytics && withdrawn.advertising);
    assert_ne!(withdrawn.revision, first.revision);
    assert_eq!(
        store
            .refresh(session(own), Some(first.revision), &grant())
            .await,
        Err(ConsentError::Conflict)
    );
    clock.0.store(2000, Ordering::SeqCst);
    let expired = store.read(session(own)).await.unwrap().unwrap();
    assert!(!expired.allows(OptionalPurpose::Advertising, 2000));
    PgAuthStore::new(pool.clone())
        .logout(own.hash)
        .await
        .unwrap();
    assert_eq!(
        store.read(session(own)).await,
        Err(ConsentError::Unauthorized)
    );
    let fresh = store
        .refresh(
            session(other),
            None,
            &Verifier {
                expiry: 3000,
                ..grant()
            },
        )
        .await
        .unwrap();
    assert!(fresh.advertising);
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(other.account)
        .execute(&pool)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM session_consents")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn concurrent_initial_refreshes_have_one_winner_and_revocation_is_authoritative() {
    let pool = auth_pool().await;
    let own = session(super::privacy::participant(&pool, 1000).await);
    let store = PgConsentStore::new(pool.clone(), Arc::new(Clock(AtomicI64::new(1100))));
    let verifier = grant();
    let (left, right) = tokio::join!(
        store.refresh(own, None, &verifier),
        store.refresh(own, None, &verifier)
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let first = left.or(right).unwrap();
    let (left, right) = tokio::join!(
        store.withdraw(own, OptionalPurpose::Analytics),
        store.withdraw(own, OptionalPurpose::Advertising)
    );
    assert!(left.is_ok() && right.is_ok());
    let denied = store.read(own).await.unwrap().unwrap();
    assert!(!denied.analytics && !denied.advertising);
    assert_eq!(
        store.refresh(own, Some(first.revision), &verifier).await,
        Err(ConsentError::Conflict)
    );
    close_auth_pool(pool).await;
}

struct DelayedVerifier {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
impl ConsentVerifier for DelayedVerifier {
    async fn verify(&self, subject: Uuid, _: i64) -> Result<VerifiedConsent, ConsentError> {
        self.entered.notify_one();
        self.release.notified().await;
        Ok(VerifiedConsent {
            subject,
            analytics: true,
            advertising: true,
            expires_at: 2000,
        })
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn a_late_verifier_cannot_overwrite_withdrawal_logout_deletion_or_time_boundaries() {
    let pool = auth_pool().await;
    for change in 0..6 {
        let own = session(super::privacy::participant(&pool, 1000).await);
        let clock = Arc::new(Clock(AtomicI64::new(1100)));
        let store = Arc::new(PgConsentStore::new(pool.clone(), clock.clone()));
        let verifier = Arc::new(DelayedVerifier {
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        let task = {
            let store = store.clone();
            let verifier = verifier.clone();
            tokio::spawn(async move { store.refresh(own, None, verifier.as_ref()).await })
        };
        tokio::time::timeout(Duration::from_secs(2), verifier.entered.notified())
            .await
            .unwrap();
        let expected = match change {
            0 => {
                // This must finish while the verifier is suspended: no DB locks are held across its IO.
                store
                    .withdraw(own, OptionalPurpose::Analytics)
                    .await
                    .unwrap();
                ConsentError::Conflict
            }
            1 => {
                PgAuthStore::new(pool.clone())
                    .logout(own.hash)
                    .await
                    .unwrap();
                ConsentError::Unauthorized
            }
            2 => {
                sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
                    .bind(own.account)
                    .execute(&pool)
                    .await
                    .unwrap();
                ConsentError::Unauthorized
            }
            3 => {
                clock.0.store(2000, Ordering::SeqCst);
                ConsentError::Invalid
            }
            4 => {
                clock.0.store(1099, Ordering::SeqCst);
                ConsentError::Invalid
            }
            _ => {
                clock.0.store(1000 + SESSION_SECONDS, Ordering::SeqCst);
                ConsentError::Unauthorized
            }
        };
        verifier.release.notify_one();
        assert_eq!(task.await.unwrap(), Err(expected), "change {change}");
        let grants: i64 = sqlx::query_scalar("SELECT count(*) FROM session_consents WHERE session_id=$1 AND (analytics OR advertising)").bind(own.revision).fetch_one(&pool).await.unwrap();
        assert_eq!(grants, 0);
    }
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn blocked_transactions_recheck_clock_session_expiry_and_real_two_second_deadline() {
    let pool = auth_pool().await;
    for change in 0..3 {
        let own = session(super::privacy::participant(&pool, 1000).await);
        let (single, pid) = super::events::single_pool(&pool).await;
        let clock = Arc::new(Clock(AtomicI64::new(1100)));
        let store = PgConsentStore::new(single.clone(), clock.clone());
        let mut held = pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
            .bind(own.account)
            .fetch_one(&mut *held)
            .await
            .unwrap();
        let task =
            tokio::spawn(async move { store.withdraw(own, OptionalPurpose::Analytics).await });
        super::events::blocked(&pool, pid).await;
        if change == 2 {
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(3), task)
                    .await
                    .unwrap()
                    .unwrap(),
                Err(ConsentError::Unavailable)
            );
            held.rollback().await.unwrap();
        } else {
            let expected = if change == 0 {
                clock.0.store(1099, Ordering::SeqCst);
                ConsentError::Invalid
            } else {
                clock.0.store(1000 + SESSION_SECONDS, Ordering::SeqCst);
                ConsentError::Unauthorized
            };
            held.commit().await.unwrap();
            assert_eq!(task.await.unwrap(), Err(expected));
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM session_consents WHERE session_id=$1"
            )
            .bind(own.revision)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
        single.close().await;
    }
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn verifier_timeout_and_closed_store_fail_without_enabling_public_collection() {
    let pool = auth_pool().await;
    let own = session(super::privacy::participant(&pool, 1000).await);
    let store = PgConsentStore::new(pool.clone(), Arc::new(Clock(AtomicI64::new(1100))));
    let verifier = DelayedVerifier {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    };
    assert_eq!(
        store.refresh(own, None, &verifier).await,
        Err(ConsentError::Unavailable)
    );
    assert_eq!(store.read(own).await, Ok(None));
    for field in 0..2 {
        let mut bad = own;
        if field == 0 {
            bad.account = Uuid::nil();
        } else {
            bad.revision = Uuid::nil();
        }
        assert_eq!(store.read(bad).await, Err(ConsentError::Unauthorized));
    }
    for path in ["/api/v1/consent", "/api/v1/events"] {
        assert_eq!(
            app(None)
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(path)
                        .body(Body::from(r#"{"analytics":true,"advertising":true}"#))
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    close_auth_pool(pool).await;
    assert_eq!(store.read(own).await, Err(ConsentError::Unavailable));
}

struct SequenceClock(std::sync::Mutex<std::collections::VecDeque<i64>>);
impl AuthClock for SequenceClock {
    fn now(&self) -> i64 {
        let mut values = self.0.lock().unwrap();
        if values.len() > 1 {
            values.pop_front().unwrap()
        } else {
            *values.front().unwrap()
        }
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn refresh_carries_the_latest_preflight_clock_and_rolls_back_expiry_before_commit() {
    let pool = auth_pool().await;
    let own = session(super::privacy::participant(&pool, 1000).await);
    // Refresh start, two preflight authorization pairs, then verifier boundary.
    let clock = SequenceClock(std::sync::Mutex::new(
        [1100, 1100, 1100, 1200, 1200, 1199].into(),
    ));
    let store = PgConsentStore::new(pool.clone(), Arc::new(clock));
    assert_eq!(
        store.refresh(own, None, &grant()).await,
        Err(ConsentError::Invalid)
    );
    // Expiry is reached after the insert, at the final authorization before commit.
    let clock = SequenceClock(std::sync::Mutex::new(
        [
            1100, 1100, 1100, 1100, 1100, 1100, 1100, 1100, 1100, 1100, 2000,
        ]
        .into(),
    ));
    let store = PgConsentStore::new(pool.clone(), Arc::new(clock));
    assert_eq!(
        store.refresh(own, None, &grant()).await,
        Err(ConsentError::Invalid)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM session_consents")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn actual_session_rotation_erases_the_old_consent_and_does_not_grant_the_new_session() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let subject = Uuid::new_v4().to_string();
    let old_token = SecretToken::generate().unwrap();
    let mut write = super::rights::account_write(Provider::Google, &subject, &old_token, 1000);
    let account = store.login(write).await.unwrap().account.id;
    let old_session = store
        .session(old_token.hash(), 1100)
        .await
        .unwrap()
        .unwrap();
    let old = ConsentSession {
        account,
        revision: old_session.id,
        hash: old_token.hash(),
    };
    let consent = PgConsentStore::new(pool.clone(), Arc::new(Clock(AtomicI64::new(1100))));
    consent.refresh(old, None, &grant()).await.unwrap();
    let new_token = SecretToken::generate().unwrap();
    write = super::rights::account_write(Provider::Google, &subject, &new_token, 1100);
    write.previous_session = Some(old_token.hash());
    assert_eq!(store.login(write).await.unwrap().account.id, account);
    let current = store
        .session(new_token.hash(), 1100)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(consent.read(old).await, Err(ConsentError::Unauthorized));
    assert_eq!(
        consent
            .read(ConsentSession {
                account,
                revision: current.id,
                hash: new_token.hash()
            })
            .await,
        Ok(None)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM session_consents")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn failed_reverification_suspends_previous_grants_during_io_and_after_timeout() {
    let pool = auth_pool().await;
    let own = session(super::privacy::participant(&pool, 1000).await);
    let store = Arc::new(PgConsentStore::new(
        pool.clone(),
        Arc::new(Clock(AtomicI64::new(1100))),
    ));
    let first = store.refresh(own, None, &grant()).await.unwrap();
    assert_eq!(
        store
            .refresh(own, Some(first.revision), &DisabledConsentVerifier)
            .await,
        Err(ConsentError::Unavailable)
    );
    let blocked = store.read(own).await.unwrap().unwrap();
    assert!(!blocked.analytics && !blocked.advertising);
    assert_ne!(blocked.revision, first.revision);
    let next = store
        .refresh(own, Some(blocked.revision), &grant())
        .await
        .unwrap();
    let verifier = Arc::new(DelayedVerifier {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let task = {
        let store = store.clone();
        let verifier = verifier.clone();
        tokio::spawn(async move {
            store
                .refresh(own, Some(next.revision), verifier.as_ref())
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(2), verifier.entered.notified())
        .await
        .unwrap();
    let during = store.read(own).await.unwrap().unwrap();
    assert!(!during.analytics && !during.advertising);
    assert_ne!(during.revision, next.revision);
    assert_eq!(task.await.unwrap(), Err(ConsentError::Unavailable));
    assert_eq!(store.read(own).await, Ok(Some(during)));
    let restored = store
        .refresh(own, Some(during.revision), &grant())
        .await
        .unwrap();
    let verifier = Arc::new(DelayedVerifier {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let task = {
        let store = store.clone();
        let verifier = verifier.clone();
        tokio::spawn(async move {
            store
                .refresh(own, Some(restored.revision), verifier.as_ref())
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(2), verifier.entered.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let cancelled = store.read(own).await.unwrap().unwrap();
    assert!(!cancelled.analytics && !cancelled.advertising);
    close_auth_pool(pool).await;
}
