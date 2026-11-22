use super::*;
use liar_protocol::{game::PublicEndReason, online::OnlineError, results::ResultStats};
use liar_server::me::{MePayload, MeQuery, MeReader, MeSession, PgMeReader};
use liar_server::online::{
    ActiveRace, AdmissionJournal, FinishedRace, PgJournalRuntime, RacePlayerResult, SaveResult,
};
use std::sync::atomic::{AtomicI64, Ordering};

const NOW: i64 = 1_800_000_000;

struct Clock(AtomicI64);

impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

async fn four_players(pool: &sqlx::PgPool) -> (ActiveRace, Vec<MeSession>) {
    let mut members = Vec::new();
    for _ in 0..4 {
        members.push(super::personal::participant(pool, NOW).await);
    }
    let active = ActiveRace {
        id: Uuid::new_v4(),
        room_id: Uuid::new_v4(),
        rules_hash: liar_core::rules::RulesSnapshot::bundled().hash.clone(),
        accounts: members.iter().map(|member| member.account).collect(),
    };
    (active, members)
}

fn final_race(active: &ActiveRace) -> FinishedRace {
    FinishedRace {
        id: active.id,
        room_id: active.room_id,
        rules_hash: active.rules_hash.clone(),
        seed: [46; 8],
        ended_ms: 20_000,
        reason: PublicEndReason::Clear,
        players: active
            .accounts
            .iter()
            .enumerate()
            .map(|(seat, &account)| RacePlayerResult {
                account,
                rank: [Some(1), Some(2), Some(2), None][seat],
                forfeited: seat == 3,
                stats: ResultStats {
                    opened_safe: [216, 130, 130, 110][seat],
                    mistakes: 0,
                    accusation_attempts: 0,
                    correct_accusations: 0,
                },
            })
            .collect(),
    }
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn race_journal_preserves_the_frozen_roster_start_and_tie_after_erasure() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(NOW)));
    let owner = PgJournalRuntime::claim(pool.clone(), clock.clone())
        .await
        .unwrap();
    let (active, members) = four_players(&pool).await;
    assert_eq!(
        owner.register_race(active.clone()).await,
        Ok(SaveResult::Saved)
    );
    assert_eq!(
        owner.register_race(active.clone()).await,
        Ok(SaveResult::Duplicate)
    );
    let mut changed = active.clone();
    changed.accounts.swap(1, 3);
    assert_eq!(
        owner.register_race(changed).await,
        Err(OnlineError::Malformed)
    );
    assert_eq!(
        owner.save_race(final_race(&active)).await,
        Err(OnlineError::Malformed)
    );
    clock.0.store(NOW + 3, Ordering::SeqCst);
    assert_eq!(owner.start_race(active.id).await, Ok(SaveResult::Saved));
    clock.0.store(NOW + 8, Ordering::SeqCst);
    assert_eq!(owner.start_race(active.id).await, Ok(SaveResult::Duplicate));
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(active.accounts[1])
        .execute(&pool)
        .await
        .unwrap();
    clock.0.store(NOW + 23, Ordering::SeqCst);
    let finished = final_race(&active);
    assert_eq!(
        owner.save_race(finished.clone()).await,
        Ok(SaveResult::Saved)
    );
    assert_eq!(
        owner.save_race(finished.clone()).await,
        Ok(SaveResult::Duplicate)
    );
    let mut wrong = finished;
    wrong.players[2].stats.opened_safe = 129;
    assert_eq!(owner.save_race(wrong).await, Err(OnlineError::Malformed));
    let saved: (Uuid, i16, i64) = sqlx::query_as("SELECT room_id,participant_count,EXTRACT(EPOCH FROM retention_started_at)::bigint FROM online_match_results WHERE id=$1")
        .bind(active.id).fetch_one(&pool).await.unwrap();
    assert_eq!(saved, (active.room_id, 4, NOW + 3));
    let rows: Vec<(i16, Option<i16>, bool, bool)> = sqlx::query_as(
        "SELECT seat,rank,tied,forfeited FROM online_match_players WHERE match_id=$1 ORDER BY seat",
    )
    .bind(active.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (0, Some(1), false, false),
            (2, Some(2), true, false),
            (3, None, false, true)
        ]
    );
    let events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM product_events WHERE online_match_id=$1")
            .bind(active.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(events, 3);
    let reader = PgMeReader::new(pool.clone(), clock.clone());
    let own = reader.read(members[2], MeQuery::Overview).await.unwrap();
    let MePayload::Overview(own) = own.payload else {
        panic!("Expected own overview");
    };
    let entry = serde_json::to_value(&own.recent_online[0]).unwrap();
    assert_eq!(
        entry["race"],
        serde_json::json!({"participant_count":4,"rank":2,"tied":true,"forfeited":false})
    );
    assert_eq!(entry["opponent"], "human");
    assert_eq!(entry["result"]["own"]["opened_safe"], 130);
    assert!(reader.read(members[1], MeQuery::Overview).await.is_err());
    let outsider = super::personal::participant(&pool, NOW).await;
    let MePayload::Overview(outsider) = reader
        .read(outsider, MeQuery::Overview)
        .await
        .unwrap()
        .payload
    else {
        panic!("Expected outsider overview");
    };
    assert!(outsider.recent_online.is_empty());
    sqlx::query("UPDATE online_match_results SET retention_started_at=to_timestamp($2::double precision) WHERE id=$1")
        .bind(active.id).bind(NOW + 23 - 7776000).execute(&pool).await.unwrap();
    let MePayload::Overview(expired) = reader
        .read(members[2], MeQuery::Overview)
        .await
        .unwrap()
        .payload
    else {
        panic!("Expected own overview after expiry");
    };
    assert!(expired.recent_online.is_empty());
    assert!(owner.health().available());
    drop(owner);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn race_journal_restart_aborts_every_surviving_seat_without_unknown_statistics() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(NOW)));
    let owner = PgJournalRuntime::claim(pool.clone(), clock.clone())
        .await
        .unwrap();
    let (active, _) = four_players(&pool).await;
    assert_eq!(
        owner.register_race(active.clone()).await,
        Ok(SaveResult::Saved)
    );
    clock.0.store(NOW + 1, Ordering::SeqCst);
    assert_eq!(owner.start_race(active.id).await, Ok(SaveResult::Saved));
    owner.fail_closed();
    drop(owner);
    clock.0.store(NOW + 9, Ordering::SeqCst);
    let recovered = tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if let Ok(owner) = PgJournalRuntime::claim(pool.clone(), clock.clone()).await {
                break owner;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let result: (Uuid, i16, String, i64, Option<i64>, Option<Vec<u8>>) = sqlx::query_as("SELECT room_id,participant_count,reason,EXTRACT(EPOCH FROM retention_started_at)::bigint,end_elapsed_ms,secret_seed FROM online_match_results WHERE id=$1")
        .bind(active.id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        result,
        (
            active.room_id,
            4,
            "server_failure".into(),
            NOW + 1,
            None,
            None
        )
    );
    let players: i64 = sqlx::query_scalar("SELECT count(*) FROM online_match_players WHERE match_id=$1 AND outcome='abort' AND rank IS NULL AND opened_safe IS NULL AND mistakes IS NULL")
        .bind(active.id).fetch_one(&pool).await.unwrap();
    assert_eq!(players, 4);
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM online_active_matches")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
    drop(recovered);
    close_auth_pool(pool).await;
}
