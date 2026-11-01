use super::{
    postgres::{db, policy},
    *,
};
use liar_protocol::daily::DailyTrust;
use sqlx::{Postgres, Row, Transaction};

pub(super) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    event_id: EventId,
    source: EventSource,
    now: i64,
) -> Result<ProductEvent, EventStoreError> {
    let (at, fact) = match source {
        EventSource::Online { account, match_id } => {
            let rows = sqlx::query("SELECT r.reason,EXTRACT(EPOCH FROM r.recorded_at)::bigint AS occurred_at,EXTRACT(EPOCH FROM COALESCE(r.retention_started_at,r.recorded_at))::bigint AS retention_at,(r.recorded_at=date_trunc('second',r.recorded_at)) AS whole_second,(SELECT count(*) FROM online_match_players WHERE match_id=r.id) AS seats,(SELECT count(*) FROM online_match_players WHERE match_id=r.id AND is_bot) AS bots FROM online_match_results r JOIN online_match_players p ON p.match_id=r.id WHERE r.id=$1 AND p.account_id=$2 AND NOT p.is_bot LIMIT 2 FOR SHARE OF r,p")
                .bind(match_id.uuid()).bind(account.uuid()).fetch_all(&mut **tx).await.map_err(db)?;
            if rows.len() > 1 {
                return Err(EventStoreError::Unavailable);
            }
            let row = rows.first().ok_or(EventStoreError::Missing)?;
            let at: i64 = row.try_get("occurred_at").map_err(db)?;
            let retention: i64 = row.try_get("retention_at").map_err(db)?;
            let seats: i64 = row.try_get("seats").map_err(db)?;
            let bots: i64 = row.try_get("bots").map_err(db)?;
            let opponent = online_source(
                at,
                retention,
                row.try_get("whole_second").map_err(db)?,
                seats,
                bots,
                now,
            )?;
            let reason = crate::results::reason(&row.try_get::<String, _>("reason").map_err(db)?)
                .map_err(|_| EventStoreError::Unavailable)?;
            (
                at,
                ServerFact::MatchEnd {
                    match_id,
                    opponent,
                    reason,
                },
            )
        }
        EventSource::Daily {
            account,
            completion_id,
        } => {
            let row = sqlx::query("SELECT c.id,c.daily_id,c.attempt_id,c.replay_digest,c.opened_safe,c.mistakes,c.replayed_elapsed_ms,c.verified_elapsed_ms,EXTRACT(EPOCH FROM c.completed_at)::bigint AS completed_at,EXTRACT(EPOCH FROM c.replay_expires_at)::bigint AS replay_expires_at,EXTRACT(EPOCH FROM c.record_expires_at)::bigint AS record_expires_at,d.id AS definition_id,to_char(d.utc_date,'YYYY-MM-DD') AS utc_date,d.seed_version,d.rules_snapshot::text AS rules_snapshot,EXTRACT(EPOCH FROM d.published_at)::bigint AS published_at,EXTRACT(EPOCH FROM d.verification_expires_at)::bigint AS verification_expires_at,EXTRACT(EPOCH FROM (d.utc_date::timestamp AT TIME ZONE 'UTC'))::bigint AS starts_at FROM daily_completions c JOIN daily_definitions d ON d.id=c.daily_id WHERE c.id=$1 AND c.account_id=$2 FOR SHARE OF c,d")
                .bind(completion_id.uuid()).bind(account.uuid()).fetch_optional(&mut **tx).await.map_err(db)?.ok_or(EventStoreError::Missing)?;
            let (_, receipt) = crate::daily::accepted_receipt(&row, now)
                .map_err(|_| EventStoreError::Unavailable)?;
            (
                receipt.completed_at,
                ServerFact::DailyVerified {
                    completion_id,
                    date: receipt.metadata.date,
                    mistakes: receipt.mistakes,
                    trust: if receipt.verified_elapsed_ms.is_some() {
                        DailyTrust::VerifiedTime
                    } else {
                        DailyTrust::VerifiedCompletion
                    },
                },
            )
        }
    };
    raw_event_alive(at, now).map_err(policy)?;
    ProductEvent::server(event_id, source.account(), at, fact).map_err(policy)
}
pub(super) fn online_source(
    at: i64,
    retention: i64,
    whole_second: bool,
    seats: i64,
    bots: i64,
    now: i64,
) -> Result<OpponentKind, EventStoreError> {
    if !whole_second || !(1..=2).contains(&seats) || !(0..=1).contains(&bots) || bots >= seats {
        return Err(EventStoreError::Unavailable);
    }
    valid_timestamp(at).map_err(policy)?;
    valid_timestamp(retention).map_err(policy)?;
    if retention > at || at > now {
        return Err(EventStoreError::Invalid);
    }
    if now - retention >= 7776000 {
        return Err(EventStoreError::Expired);
    }
    Ok(if bots == 1 {
        OpponentKind::Bot
    } else {
        OpponentKind::Human
    })
}
