use super::{optional_model::admit_current, *};
use crate::{
    auth::AuthClock,
    consent::{self, ConsentError},
};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use tokio::{sync::Semaphore, time::Instant};

pub struct PgOptionalEventStore {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
    requests: Arc<Semaphore>,
}
impl PgOptionalEventStore {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self {
            pool,
            clock,
            requests: Arc::new(Semaphore::new(4)),
        }
    }
}
impl OptionalEventStore for PgOptionalEventStore {
    async fn capture(&self, input: OptionalCapture) -> Result<StoredEvent, OptionalEventError> {
        let _permit = self
            .requests
            .clone()
            .try_acquire_owned()
            .map_err(|_| OptionalEventError::Unavailable)?;
        let deadline = Instant::now() + Duration::from_secs(2);
        tokio::time::timeout_at(deadline, async {
            if input.expected_revision.is_nil() {
                return Err(OptionalEventError::Invalid);
            }
            let started = postgres::checked_now(self.clock.as_ref(), 0).map_err(storage)?;
            let mut tx = self.pool.begin().await.map_err(db)?;
            crate::postgres_deadline::arm(&mut tx, deadline)
                .await
                .map_err(|_| OptionalEventError::Unavailable)?;
            let (now, expiry) =
                consent::authorize_in_transaction(&mut tx, input.session, &self.clock, started)
                    .await
                    .map_err(authority)?;
            let current =
                consent::current_in_transaction(&mut tx, input.session.revision, now, expiry)
                    .await
                    .map_err(authority)?;
            let event = admit_current(
                input.draft.clone(),
                input.session.revision,
                input.expected_revision,
                current,
                now,
            )
            .map_err(policy)?;
            let received = postgres::checked_now(self.clock.as_ref(), now).map_err(storage)?;
            let stored = super::optional_records::insert(&mut tx, &input, event, received).await?;
            let (finish, expiry) =
                consent::authorize_in_transaction(&mut tx, input.session, &self.clock, received)
                    .await
                    .map_err(authority)?;
            let current =
                consent::current_in_transaction(&mut tx, input.session.revision, finish, expiry)
                    .await
                    .map_err(authority)?;
            let commit_at = postgres::checked_now(self.clock.as_ref(), finish).map_err(storage)?;
            if commit_at >= expiry {
                return Err(OptionalEventError::Unauthorized);
            }
            admit_current(
                input.draft,
                input.session.revision,
                input.expected_revision,
                current,
                commit_at,
            )
            .map_err(policy)?;
            tx.commit().await.map_err(db)?;
            Ok(stored)
        })
        .await
        .unwrap_or(Err(OptionalEventError::Unavailable))
    }
}
pub(super) fn db(_: sqlx::Error) -> OptionalEventError {
    OptionalEventError::Unavailable
}
fn authority(value: ConsentError) -> OptionalEventError {
    match value {
        ConsentError::Unauthorized => OptionalEventError::Unauthorized,
        _ => OptionalEventError::Unavailable,
    }
}
fn storage(value: EventStoreError) -> OptionalEventError {
    match value {
        EventStoreError::Invalid => OptionalEventError::Invalid,
        _ => OptionalEventError::Unavailable,
    }
}
pub(super) fn policy(value: EventError) -> OptionalEventError {
    match value {
        EventError::Consent => OptionalEventError::Consent,
        EventError::Expired => OptionalEventError::Expired,
        _ => OptionalEventError::Invalid,
    }
}
