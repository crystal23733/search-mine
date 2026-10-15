use super::{
    model::EventContent,
    postgres::{db, policy},
    *,
};
use liar_protocol::{
    daily::DailyTrust,
    privacy::{EventOpponent, ExportEventFact, ExportRecord},
};
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};
use uuid::Uuid;

const COLUMNS: &str = "id,account_id,purpose,occurred_at,received_at,online_match_id,daily_completion_id,opponent_type,end_reason,to_char(utc_date,'YYYY-MM-DD') AS utc_date,mistakes,trust";
fn name<T: serde::Serialize>(value: T) -> Result<String, EventStoreError> {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .ok_or(EventStoreError::Unavailable)
}
fn event_id(id: Uuid) -> Result<EventId, EventStoreError> {
    EventId::parse(&id.to_string()).map_err(|_| EventStoreError::Unavailable)
}
fn hydrate(row: &PgRow) -> Result<StoredEvent, EventStoreError> {
    let at: i64 = row.try_get("occurred_at").map_err(db)?;
    let received_at: i64 = row.try_get("received_at").map_err(db)?;
    raw_event_alive(at, received_at).map_err(|_| EventStoreError::Unavailable)?;
    if row.try_get::<String, _>("purpose").map_err(db)? != "essential" {
        return Err(EventStoreError::Unavailable);
    }
    let online: Option<Uuid> = row.try_get("online_match_id").map_err(db)?;
    let daily: Option<Uuid> = row.try_get("daily_completion_id").map_err(db)?;
    let opponent: Option<String> = row.try_get("opponent_type").map_err(db)?;
    let reason: Option<String> = row.try_get("end_reason").map_err(db)?;
    let date: Option<String> = row.try_get("utc_date").map_err(db)?;
    let mistakes: Option<i32> = row.try_get("mistakes").map_err(db)?;
    let trust: Option<String> = row.try_get("trust").map_err(db)?;
    let fact = match (online, daily, opponent, reason, date, mistakes, trust) {
        (Some(match_id), None, Some(opponent), Some(reason), None, None, None) => {
            ServerFact::MatchEnd {
                match_id: event_id(match_id)?,
                opponent: match opponent.as_str() {
                    "human" => OpponentKind::Human,
                    "bot" => OpponentKind::Bot,
                    _ => return Err(EventStoreError::Unavailable),
                },
                reason: crate::results::reason(&reason)
                    .map_err(|_| EventStoreError::Unavailable)?,
            }
        }
        (None, Some(completion_id), None, None, Some(date), Some(mistakes), Some(trust)) => {
            ServerFact::DailyVerified {
                completion_id: event_id(completion_id)?,
                date,
                mistakes: u16::try_from(mistakes).map_err(|_| EventStoreError::Unavailable)?,
                trust: match trust.as_str() {
                    "verified_completion" => DailyTrust::VerifiedCompletion,
                    "verified_time" => DailyTrust::VerifiedTime,
                    _ => return Err(EventStoreError::Unavailable),
                },
            }
        }
        _ => return Err(EventStoreError::Unavailable),
    };
    Ok(StoredEvent {
        event: ProductEvent::server(
            event_id(row.try_get("id").map_err(db)?)?,
            event_id(row.try_get("account_id").map_err(db)?)?,
            at,
            fact,
        )
        .map_err(|_| EventStoreError::Unavailable)?,
        received_at,
        inserted: false,
    })
}
pub(super) async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    event: ProductEvent,
    received_at: i64,
) -> Result<StoredEvent, EventStoreError> {
    let EventSubject::Account(account) = event.subject else {
        return Err(EventStoreError::Invalid);
    };
    let (online, daily, opponent, reason, date, mistakes, trust) = match &event.content {
        EventContent::Server(ServerFact::MatchEnd {
            match_id,
            opponent,
            reason,
        }) => (
            Some(match_id.uuid()),
            None,
            Some(name(opponent)?),
            Some(name(reason)?),
            None,
            None,
            None,
        ),
        EventContent::Server(ServerFact::DailyVerified {
            completion_id,
            date,
            mistakes,
            trust,
        }) => (
            None,
            Some(completion_id.uuid()),
            None,
            None,
            Some(date.clone()),
            Some(i32::from(*mistakes)),
            Some(name(trust)?),
        ),
        _ => return Err(EventStoreError::Invalid),
    };
    let inserted = sqlx::query("INSERT INTO product_events(id,account_id,occurred_at,received_at,online_match_id,daily_completion_id,opponent_type,end_reason,utc_date,mistakes,trust) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9::text::date,$10,$11) ON CONFLICT DO NOTHING")
        .bind(event.id().uuid()).bind(account.uuid()).bind(event.occurred_at()).bind(received_at).bind(online).bind(daily).bind(opponent).bind(reason).bind(date).bind(mistakes).bind(trust).execute(&mut **tx).await.map_err(db)?.rows_affected()==1;
    let mut query = sqlx::QueryBuilder::new("SELECT ");
    let rows = query
        .push(COLUMNS)
        .push(" FROM product_events WHERE id=")
        .push_bind(event.id().uuid())
        .push(" OR (account_id=")
        .push_bind(account.uuid())
        .push(" AND (online_match_id=")
        .push_bind(online)
        .push(" OR daily_completion_id=")
        .push_bind(daily)
        .push(")) ORDER BY (id=")
        .push_bind(event.id().uuid())
        .push(") DESC LIMIT 2")
        .build()
        .fetch_all(&mut **tx)
        .await
        .map_err(db)?;
    let mut stored = hydrate(rows.first().ok_or(EventStoreError::Unavailable)?)?;
    let mut expected = event.clone();
    if stored.event.id() != event.id() {
        expected.event_id = stored.event.id();
    }
    if expected != stored.event {
        return Err(EventStoreError::Conflict);
    }
    stored.inserted = inserted;
    Ok(stored)
}
fn export(stored: StoredEvent) -> Result<ExportRecord, EventStoreError> {
    let fact = match stored.event.content {
        EventContent::Server(ServerFact::MatchEnd {
            match_id,
            opponent,
            reason,
        }) => ExportEventFact::MatchEnd {
            match_id: match_id.uuid().to_string(),
            opponent: match opponent {
                OpponentKind::Human => EventOpponent::Human,
                OpponentKind::Bot => EventOpponent::Bot,
            },
            reason,
        },
        EventContent::Server(ServerFact::DailyVerified {
            completion_id,
            date,
            mistakes,
            trust,
        }) => ExportEventFact::DailyVerified {
            completion_id: completion_id.uuid().to_string(),
            date,
            mistakes,
            trust,
        },
        _ => return Err(EventStoreError::Unavailable),
    };
    Ok(ExportRecord::Event {
        event_id: stored.event.event_id.uuid().to_string(),
        occurred_at: stored.event.occurred_at,
        received_at: stored.received_at,
        fact,
    })
}
pub(super) async fn metric_events(
    tx: &mut Transaction<'_, Postgres>,
    start: i64,
    end: i64,
    now: i64,
) -> Result<Vec<ProductEvent>, super::MetricReadError> {
    let mut query = sqlx::QueryBuilder::new("SELECT ");
    let rows=query.push(COLUMNS)
        .push(" FROM product_events WHERE online_match_id IS NOT NULL AND account_id IN (SELECT id FROM auth_accounts) AND occurred_at>=")
        .push_bind(start).push(" AND occurred_at<").push_bind(end)
        .push(" ORDER BY occurred_at,id LIMIT ").push_bind((super::MAX_METRIC_INPUTS+1) as i64)
        .build().fetch_all(&mut **tx).await.map_err(|_|super::MetricReadError::Unavailable)?;
    if rows.len() > super::MAX_METRIC_INPUTS {
        return Err(super::EventError::Capacity.into());
    }
    rows.iter()
        .map(|row| {
            let stored = hydrate(row).map_err(|_| super::MetricReadError::Unavailable)?;
            super::raw_event_alive(stored.event.occurred_at(), now)?;
            if stored.received_at > now {
                return Err(super::EventError::Invalid.into());
            }
            Ok(stored.event)
        })
        .collect()
}
pub(crate) async fn export_events(
    tx: &mut Transaction<'_, Postgres>,
    account: Uuid,
    after: Option<Uuid>,
    now: i64,
) -> Result<Vec<ExportRecord>, EventStoreError> {
    valid_timestamp(now).map_err(policy)?;
    let mut query = sqlx::QueryBuilder::new("SELECT ");
    let rows = query
        .push(COLUMNS)
        .push(" FROM product_events WHERE account_id=")
        .push_bind(account)
        .push(" AND (")
        .push_bind(after)
        .push("::uuid IS NULL OR id>")
        .push_bind(after)
        .push(") AND occurred_at>")
        .push_bind(now - RAW_EVENT_SECONDS)
        .push(" ORDER BY id LIMIT 51")
        .build()
        .fetch_all(&mut **tx)
        .await
        .map_err(db)?;
    rows.iter()
        .map(|row| {
            let stored = hydrate(row)?;
            raw_event_alive(stored.event.occurred_at(), now).map_err(policy)?;
            if stored.received_at > now {
                return Err(EventStoreError::Invalid);
            }
            export(stored)
        })
        .collect()
}
