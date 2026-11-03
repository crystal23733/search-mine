use super::*;
use crate::auth::{AuthClock, AuthError, SessionAuthority, authorize_account_transaction};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::{future::Future, sync::Arc};
use tokio::time::Instant;
use uuid::Uuid;

pub struct PgConsentStore {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
    deadline: Option<Instant>,
}
impl PgConsentStore {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self {
            pool,
            clock,
            deadline: None,
        }
    }
    fn for_deadline(&self, deadline: std::time::Instant) -> Self {
        Self {
            pool: self.pool.clone(),
            clock: self.clock.clone(),
            deadline: Some(Instant::from_std(deadline)),
        }
    }
    async fn transaction(
        &self,
        deadline: Instant,
    ) -> Result<Transaction<'static, Postgres>, ConsentError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        crate::postgres_deadline::arm(&mut tx, deadline)
            .await
            .map_err(|_| ConsentError::Unavailable)?;
        Ok(tx)
    }
    async fn authorize(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        session: ConsentSession,
        previous: i64,
    ) -> Result<(i64, i64), ConsentError> {
        authorize_in_transaction(tx, session, &self.clock, previous).await
    }
    async fn read_at(
        &self,
        session: ConsentSession,
        previous: i64,
    ) -> Result<(Option<CurrentConsent>, i64), ConsentError> {
        bounded(self.deadline, |deadline| async move {
            let started = model::checked_now(&self.clock, previous)?;
            let mut tx = self.transaction(deadline).await?;
            let (now, expiry) = self.authorize(&mut tx, session, started).await?;
            let current = load(&mut tx, session.revision).await?;
            model::validate(session.revision, current, now, expiry)?;
            let (finish, _) = self.authorize(&mut tx, session, now).await?;
            tx.commit().await.map_err(db)?;
            Ok((current, finish))
        })
        .await
    }
    pub async fn read(
        &self,
        session: ConsentSession,
    ) -> Result<Option<CurrentConsent>, ConsentError> {
        self.read_at(session, 0).await.map(|value| value.0)
    }
    pub async fn refresh(
        &self,
        session: ConsentSession,
        expected: Option<Uuid>,
        verifier: &impl ConsentVerifier,
    ) -> Result<CurrentConsent, ConsentError> {
        bounded(self.deadline, |deadline| async move {
            let started = model::checked_now(&self.clock, 0)?;
            // Commit a blocked revision before verifier IO so failure cannot preserve an old grant.
            let (expected, observed_at) = self
                .begin_refresh(session, expected, started, deadline)
                .await?;
            let before_verify = model::checked_now(&self.clock, observed_at)?;
            let evidence = verifier.verify(session.revision, before_verify).await?;
            let after_verify = model::checked_now(&self.clock, before_verify)?;
            let mut tx = self.transaction(deadline).await?;
            let (now, expiry) = self.authorize(&mut tx, session, after_verify).await?;
            let current = load(&mut tx, session.revision).await?;
            let value =
                model::refreshed(session.revision, expected, current, evidence, now, expiry)?;
            save(&mut tx, value).await?;
            let (finish, _) = self.authorize(&mut tx, session, now).await?;
            if finish >= value.expires_at {
                return Err(ConsentError::Invalid);
            }
            tx.commit().await.map_err(db)?;
            Ok(value)
        })
        .await
    }
    async fn begin_refresh(
        &self,
        session: ConsentSession,
        expected: Option<Uuid>,
        previous: i64,
        deadline: Instant,
    ) -> Result<(Option<Uuid>, i64), ConsentError> {
        let mut tx = self.transaction(deadline).await?;
        let (now, expiry) = self.authorize(&mut tx, session, previous).await?;
        let current = load(&mut tx, session.revision).await?;
        let blocked = model::suspended(session.revision, expected, current, now, expiry)?;
        if let Some(value) = blocked {
            save(&mut tx, value).await?;
        }
        let (finish, _) = self.authorize(&mut tx, session, now).await?;
        tx.commit().await.map_err(db)?;
        Ok((blocked.map(|value| value.revision), finish))
    }
    pub async fn withdraw(
        &self,
        session: ConsentSession,
        purpose: OptionalPurpose,
    ) -> Result<CurrentConsent, ConsentError> {
        self.withdraw_selected(session, Some(purpose)).await
    }
    pub async fn withdraw_all(
        &self,
        session: ConsentSession,
    ) -> Result<CurrentConsent, ConsentError> {
        self.withdraw_selected(session, None).await
    }
    async fn withdraw_selected(
        &self,
        session: ConsentSession,
        purpose: Option<OptionalPurpose>,
    ) -> Result<CurrentConsent, ConsentError> {
        bounded(self.deadline, |deadline| async move {
            let started = model::checked_now(&self.clock, 0)?;
            let mut tx = self.transaction(deadline).await?;
            let (now, expiry) = self.authorize(&mut tx, session, started).await?;
            let current = load(&mut tx, session.revision).await?;
            let value = match purpose {
                Some(purpose) => model::withdrawn(session.revision, current, purpose, now, expiry)?,
                None => model::withdrawn_all(session.revision, current, now, expiry)?,
            };
            save(&mut tx, value).await?;
            self.authorize(&mut tx, session, now).await?;
            tx.commit().await.map_err(db)?;
            Ok(value)
        })
        .await
    }
}
impl ConsentStore for PgConsentStore {
    async fn read(
        &self,
        session: ConsentSession,
        deadline: std::time::Instant,
    ) -> Result<Option<CurrentConsent>, ConsentError> {
        self.for_deadline(deadline).read(session).await
    }
    async fn refresh(
        &self,
        session: ConsentSession,
        expected: Option<Uuid>,
        verifier: &impl ConsentVerifier,
        deadline: std::time::Instant,
    ) -> Result<CurrentConsent, ConsentError> {
        self.for_deadline(deadline)
            .refresh(session, expected, verifier)
            .await
    }
    async fn withdraw(
        &self,
        session: ConsentSession,
        purpose: OptionalPurpose,
        deadline: std::time::Instant,
    ) -> Result<CurrentConsent, ConsentError> {
        self.for_deadline(deadline).withdraw(session, purpose).await
    }
    async fn withdraw_all(
        &self,
        session: ConsentSession,
        deadline: std::time::Instant,
    ) -> Result<CurrentConsent, ConsentError> {
        self.for_deadline(deadline).withdraw_all(session).await
    }
}
/// Locks and validates the account/session shared by consent and optional writes.
pub(crate) async fn authorize_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    session: ConsentSession,
    clock: &Arc<dyn AuthClock>,
    previous: i64,
) -> Result<(i64, i64), ConsentError> {
    if session.account.is_nil() || session.revision.is_nil() {
        return Err(ConsentError::Unauthorized);
    }
    let started = model::checked_now(clock, previous)?;
    authorize_account_transaction(
        tx,
        SessionAuthority {
            account: session.account,
            hash: session.hash,
            now: started,
        },
        false,
    )
    .await
    .map_err(auth)?;
    let row = sqlx::query("SELECT id,EXTRACT(EPOCH FROM created_at)::bigint AS created_at,EXTRACT(EPOCH FROM authenticated_at)::bigint AS authenticated_at,EXTRACT(EPOCH FROM expires_at)::bigint AS expires_at FROM auth_sessions WHERE account_id=$1 AND token_hash=$2")
            .bind(session.account).bind(session.hash.as_slice()).fetch_optional(&mut **tx).await.map_err(db)?.ok_or(ConsentError::Unauthorized)?;
    let now = model::checked_now(clock, started)?;
    let id: Uuid = row.try_get("id").map_err(db)?;
    let created: i64 = row.try_get("created_at").map_err(db)?;
    let authenticated: i64 = row.try_get("authenticated_at").map_err(db)?;
    let expiry: i64 = row.try_get("expires_at").map_err(db)?;
    if id != session.revision
        || created < 0
        || created > now
        || authenticated < created
        || authenticated > now
        || expiry <= now
    {
        return Err(ConsentError::Unauthorized);
    }
    model::validate(session.revision, None, now, expiry)?;
    Ok((now, expiry))
}

async fn load(
    tx: &mut Transaction<'_, Postgres>,
    subject: Uuid,
) -> Result<Option<CurrentConsent>, ConsentError> {
    let row = sqlx::query("SELECT session_id,revision,checked_at,expires_at,analytics,advertising FROM session_consents WHERE session_id=$1")
        .bind(subject).fetch_optional(&mut **tx).await.map_err(db)?;
    row.map(|row| {
        Ok(CurrentConsent {
            subject: row.try_get("session_id").map_err(db)?,
            revision: row.try_get("revision").map_err(db)?,
            checked_at: row.try_get("checked_at").map_err(db)?,
            expires_at: row.try_get("expires_at").map_err(db)?,
            analytics: row.try_get("analytics").map_err(db)?,
            advertising: row.try_get("advertising").map_err(db)?,
        })
    })
    .transpose()
}
/// Reads within the caller's already authorized account/session transaction.
pub(crate) async fn current_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    subject: Uuid,
    now: i64,
    session_expiry: i64,
) -> Result<Option<CurrentConsent>, ConsentError> {
    let current = load(tx, subject).await?;
    model::validate(subject, current, now, session_expiry)?;
    Ok(current)
}
async fn save(
    tx: &mut Transaction<'_, Postgres>,
    value: CurrentConsent,
) -> Result<(), ConsentError> {
    sqlx::query("INSERT INTO session_consents(session_id,revision,checked_at,expires_at,analytics,advertising) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(session_id) DO UPDATE SET revision=EXCLUDED.revision,checked_at=EXCLUDED.checked_at,expires_at=EXCLUDED.expires_at,analytics=EXCLUDED.analytics,advertising=EXCLUDED.advertising")
        .bind(value.subject).bind(value.revision).bind(value.checked_at).bind(value.expires_at).bind(value.analytics).bind(value.advertising).execute(&mut **tx).await.map_err(db)?;
    Ok(())
}
fn db(_: sqlx::Error) -> ConsentError {
    ConsentError::Unavailable
}
fn auth(error: AuthError) -> ConsentError {
    match error {
        AuthError::Unavailable => ConsentError::Unavailable,
        _ => ConsentError::Unauthorized,
    }
}
async fn bounded<T, F: Future<Output = Result<T, ConsentError>>>(
    deadline: Option<Instant>,
    operation: impl FnOnce(Instant) -> F,
) -> Result<T, ConsentError> {
    let now = Instant::now();
    let limit = now + CONSENT_REQUEST_TIMEOUT;
    let deadline = deadline.unwrap_or(limit);
    if deadline <= now || deadline > limit {
        return Err(ConsentError::Unavailable);
    }
    tokio::time::timeout_at(deadline, operation(deadline))
        .await
        .unwrap_or(Err(ConsentError::Unavailable))
}
