use super::*;
use crate::auth::{AuthClock, AuthError, SessionAuthority, authorize_account_transaction};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::{future::Future, sync::Arc, time::Duration};
use uuid::Uuid;

pub struct PgConsentStore {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
}
impl PgConsentStore {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self { pool, clock }
    }
    async fn authorize(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        session: ConsentSession,
        previous: i64,
    ) -> Result<(i64, i64), ConsentError> {
        if session.account.is_nil() || session.revision.is_nil() {
            return Err(ConsentError::Unauthorized);
        }
        let started = model::checked_now(&self.clock, previous)?;
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
        let now = model::checked_now(&self.clock, started)?;
        let id: Uuid = row.try_get("id").map_err(db)?;
        let created: i64 = row.try_get("created_at").map_err(db)?;
        let authenticated: i64 = row.try_get("authenticated_at").map_err(db)?;
        let expiry: i64 = row.try_get("expires_at").map_err(db)?;
        if id != session.revision
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
    async fn read_at(
        &self,
        session: ConsentSession,
        previous: i64,
    ) -> Result<(Option<CurrentConsent>, i64), ConsentError> {
        bounded(async {
            let started = model::checked_now(&self.clock, previous)?;
            let mut tx = self.pool.begin().await.map_err(db)?;
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
        bounded(async {
            let started = model::checked_now(&self.clock, 0)?;
            // Authorize before contacting an external verifier, without holding DB locks during IO.
            let (current, observed_at) = self.read_at(session, started).await?;
            if current.map(|value| value.revision) != expected {
                return Err(ConsentError::Conflict);
            }
            let before_verify = model::checked_now(&self.clock, observed_at)?;
            let evidence = verifier.verify(session.revision, before_verify).await?;
            let after_verify = model::checked_now(&self.clock, before_verify)?;
            let mut tx = self.pool.begin().await.map_err(db)?;
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
    pub async fn withdraw(
        &self,
        session: ConsentSession,
        purpose: OptionalPurpose,
    ) -> Result<CurrentConsent, ConsentError> {
        bounded(async {
            let started = model::checked_now(&self.clock, 0)?;
            let mut tx = self.pool.begin().await.map_err(db)?;
            let (now, expiry) = self.authorize(&mut tx, session, started).await?;
            let current = load(&mut tx, session.revision).await?;
            let value = model::withdrawn(session.revision, current, purpose, now, expiry)?;
            save(&mut tx, value).await?;
            self.authorize(&mut tx, session, now).await?;
            tx.commit().await.map_err(db)?;
            Ok(value)
        })
        .await
    }
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
async fn bounded<T>(
    operation: impl Future<Output = Result<T, ConsentError>>,
) -> Result<T, ConsentError> {
    tokio::time::timeout(Duration::from_secs(2), operation)
        .await
        .unwrap_or(Err(ConsentError::Unavailable))
}
