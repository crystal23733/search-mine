use super::*;
use liar_protocol::results::ResultError;
use liar_server::{
    online::{PgResultRepository, ResultRepository},
    results::{PgResultReader, ResultReader},
};
use std::sync::atomic::{AtomicI64, Ordering};

struct Clock(AtomicI64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
async fn add(
    pool: &sqlx::PgPool,
    clock: &Arc<Clock>,
    players: [Option<Uuid>; 2],
    at: i64,
    anchor: Option<i64>,
    id: Uuid,
) {
    clock.0.store(at, Ordering::SeqCst);
    let mut final_match = super::online::finished(players);
    final_match.id = id;
    // Preserve a historical hash, independently of the running bundled rules.
    final_match.rules_hash = "a".repeat(64);
    PgResultRepository::new(pool.clone(), clock.clone())
        .save(final_match)
        .await
        .unwrap();
    sqlx::query("UPDATE online_match_results SET retention_started_at=to_timestamp($1::double precision) WHERE id=$2")
        .bind(anchor).bind(id).execute(pool).await.unwrap();
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn latest_is_self_only_uses_play_order_not_recovery_order_and_keeps_legacy_anchor() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let a = super::online::participant(&store).await;
    let b = super::online::participant(&store).await;
    let clock = Arc::new(Clock(AtomicI64::new(20000)));
    let reader = PgResultReader::new(pool.clone(), clock.clone());
    assert!(reader.latest(a).await.unwrap().is_none());
    let recent = Uuid::new_v4();
    let old = Uuid::new_v4();
    add(&pool, &clock, [None, Some(a)], 19000, None, recent).await;
    add(&pool, &clock, [Some(a), None], 19999, Some(18000), old).await;
    add(
        &pool,
        &clock,
        [Some(b), None],
        20000,
        Some(20000),
        Uuid::new_v4(),
    )
    .await;
    let found = reader.latest(a).await.unwrap().unwrap();
    assert_eq!(found.match_id, recent);
    assert_eq!(found.account, a);
    assert_eq!(found.rules_hash, "a".repeat(64));
    let own = found.project(a, recent).unwrap();
    assert_eq!(own.match_id, recent.to_string());
    assert_eq!(own.end_elapsed_ms, Some(243000));
    assert_eq!(own.own.unwrap().opened_safe, 4);
    sqlx::query("DELETE FROM online_match_players WHERE match_id=$1 AND account_id=$2")
        .bind(recent)
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(reader.latest(a).await.unwrap().unwrap().match_id, old);
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();
    assert!(reader.latest(a).await.unwrap().is_none());
    assert!(reader.latest(b).await.unwrap().is_some());
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn latest_retention_has_exact_elapsed_boundary_and_rejects_invalid_subject_or_clock() {
    let pool = auth_pool().await;
    let a = super::online::participant(&PgAuthStore::new(pool.clone())).await;
    let now = 8_000_000_i64;
    let cutoff = now - 7_776_000;
    let id = Uuid::new_v4();
    let clock = Arc::new(Clock(AtomicI64::new(now)));
    let reader = PgResultReader::new(pool.clone(), clock.clone());
    add(&pool, &clock, [Some(a), None], now, Some(cutoff + 1), id).await;
    assert_eq!(reader.latest(a).await.unwrap().unwrap().match_id, id);
    for anchor in [cutoff, cutoff - 1, 0] {
        sqlx::query("UPDATE online_match_results SET retention_started_at=to_timestamp($1::double precision) WHERE id=$2").bind(anchor).bind(id).execute(&pool).await.unwrap();
        assert!(reader.latest(a).await.unwrap().is_none());
    }
    sqlx::query("UPDATE online_match_results SET recorded_at=to_timestamp($1::double precision),retention_started_at=to_timestamp($1::double precision) WHERE id=$2").bind(now+1).bind(id).execute(&pool).await.unwrap();
    assert!(reader.latest(a).await.unwrap().is_none());
    assert!(matches!(
        reader.latest(Uuid::nil()).await,
        Err(ResultError::Unavailable)
    ));
    for invalid in [-1, 253402300800] {
        clock.0.store(invalid, Ordering::SeqCst);
        assert!(matches!(
            reader.latest(a).await,
            Err(ResultError::Unavailable)
        ));
    }
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn latest_ties_are_stable_and_corrupt_or_ambiguous_newest_does_not_fall_back() {
    let pool = auth_pool().await;
    let a = super::online::participant(&PgAuthStore::new(pool.clone())).await;
    let clock = Arc::new(Clock(AtomicI64::new(20000)));
    let reader = PgResultReader::new(pool.clone(), clock.clone());
    let old = Uuid::from_u128(1);
    let newest = Uuid::from_u128(2);
    add(&pool, &clock, [Some(a), None], 20000, Some(19000), old).await;
    add(&pool, &clock, [Some(a), None], 20000, Some(19000), newest).await;
    for _ in 0..3 {
        assert_eq!(reader.latest(a).await.unwrap().unwrap().match_id, newest);
    }
    sqlx::query("UPDATE online_match_results SET recorded_at=to_timestamp(19999) WHERE id=$1")
        .bind(newest)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(reader.latest(a).await.unwrap().unwrap().match_id, old);
    sqlx::query("UPDATE online_match_results SET recorded_at=to_timestamp(20000) WHERE id=$1")
        .bind(newest)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE online_match_results SET reason='server_failure' WHERE id=$1")
        .bind(newest)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        reader.latest(a).await,
        Err(ResultError::Unavailable)
    ));
    sqlx::query("UPDATE online_match_results SET reason='timeout' WHERE id=$1")
        .bind(newest)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE online_match_players SET account_id=$1,is_bot=false WHERE match_id=$2 AND seat=1",
    )
    .bind(a)
    .bind(newest)
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        reader.latest(a).await,
        Err(ResultError::Unavailable)
    ));
    close_auth_pool(pool).await;
}
