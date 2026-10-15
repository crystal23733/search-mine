use super::{EventError, EventId, ProductEvent, raw_event_alive, valid_timestamp};
use crate::auth::AuthClock;
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};

#[derive(Clone, Copy, Debug)]
pub enum EventSource {
    Online {
        account: EventId,
        match_id: EventId,
    },
    Daily {
        account: EventId,
        completion_id: EventId,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventStoreError {
    Invalid,
    Expired,
    Conflict,
    Missing,
    Unavailable,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredEvent {
    pub event: ProductEvent,
    pub received_at: i64,
    pub inserted: bool,
}
pub struct PgEventStore {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
}
impl PgEventStore {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self { pool, clock }
    }
    pub async fn capture(
        &self,
        event_id: EventId,
        source: EventSource,
    ) -> Result<StoredEvent, EventStoreError> {
        tokio::time::timeout(Duration::from_secs(2), async {
            let started = checked_now(self.clock.as_ref(), 0)?;
            let mut tx = self.pool.begin().await.map_err(db)?;
            let account = source.account();
            let live: Option<uuid::Uuid> = sqlx::query_scalar("SELECT id FROM auth_accounts WHERE id=$1 AND created_at<=to_timestamp($2::double precision) FOR UPDATE")
                .bind(account.uuid()).bind(started).fetch_optional(&mut *tx).await.map_err(db)?;
            if live.is_none() { return Err(EventStoreError::Missing); }
            let now = checked_now(self.clock.as_ref(), started)?;
            let event = super::pg_source::load(&mut tx, event_id, source, now).await?;
            let received_at = checked_now(self.clock.as_ref(), now)?;
            raw_event_alive(event.occurred_at(), received_at).map_err(policy)?;
            let result = super::pg_records::insert(&mut tx, event, received_at).await?;
            let finish = checked_now(self.clock.as_ref(), received_at)?;
            raw_event_alive(result.event.occurred_at(), finish).map_err(policy)?;
            tx.commit().await.map_err(db)?;
            Ok(result)
        }).await.unwrap_or(Err(EventStoreError::Unavailable))
    }
}
impl EventSource {
    pub(super) fn account(self) -> EventId {
        match self {
            Self::Online { account, .. } | Self::Daily { account, .. } => account,
        }
    }
}
fn checked_now(clock: &dyn AuthClock, previous: i64) -> Result<i64, EventStoreError> {
    let now = clock.now();
    valid_timestamp(now).map_err(policy)?;
    if now < previous {
        return Err(EventStoreError::Invalid);
    }
    Ok(now)
}
pub(super) fn db(_: sqlx::Error) -> EventStoreError {
    EventStoreError::Unavailable
}
pub(super) fn policy(error: EventError) -> EventStoreError {
    match error {
        EventError::Expired => EventStoreError::Expired,
        _ => EventStoreError::Invalid,
    }
}
