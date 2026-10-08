use super::postgres::{database_error, timestamp};
use super::postgres_accounts::{find_identity, lock_account, validate_digests};
use super::postgres_rights::{erase_locked, revoke_locked};
use super::*;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;
impl AppleMaintenanceStore for PgAuthStore {
    async fn apply_notification(&self, event: AppleNoticeWrite) -> Result<Option<Uuid>, AuthError> {
        let now = timestamp(event.now)?;
        if event.occurred_at < 0
            || event.occurred_at > event.now.saturating_add(30)
            || event.occurred_at < event.now.saturating_sub(86400)
        {
            return Err(AuthError::Invalid);
        }
        let mut tx = self.pool.begin().await.map_err(database_error)?;
        let inserted=sqlx::query("INSERT INTO auth_apple_notification_receipts(jti_digest,expires_at) VALUES($1,to_timestamp($2)+interval '28 days') ON CONFLICT DO NOTHING").bind(event.jti_hash.as_slice()).bind(now).execute(&mut *tx).await.map_err(database_error)?;
        let mut affected = None;
        if inserted.rows_affected() == 1
            && let Some(digests) = event.digests
            && let Some((identity, account)) =
                find_identity(&mut tx, Provider::Apple, &digests).await?
        {
            match lock_account(&mut tx, account).await {
                Ok(_) => {}
                Err(AuthError::Unauthenticated) => {
                    tx.commit().await.map_err(database_error)?;
                    return Ok(None);
                }
                Err(e) => return Err(e),
            }
            let effective=sqlx::query_scalar::<_,i64>("SELECT EXTRACT(EPOCH FROM GREATEST(i.linked_at,COALESCE(c.updated_at,i.linked_at)))::bigint FROM auth_identities i LEFT JOIN auth_credentials c ON c.identity_id=i.id WHERE i.id=$1 AND i.account_id=$2").bind(identity).bind(account).fetch_optional(&mut *tx).await.map_err(database_error)?;
            if effective.is_some_and(|at| at <= event.occurred_at) {
                detach_apple(&mut tx, identity, account, event.now).await?;
                affected = Some(account);
            }
        }
        let _authority_barrier = affected.map(|account| self.invalidations.account(account));
        tx.commit().await.map_err(database_error)?;
        Ok(affected)
    }
    async fn claim_revoke(&self, now: i64) -> Result<Option<RevokeJob>, AuthError> {
        let lease = Uuid::new_v4();
        let row=sqlx::query("WITH candidate AS (SELECT id FROM auth_apple_revoke_queue WHERE expires_at>to_timestamp($1) AND next_attempt_at<=to_timestamp($1) AND (lease_id IS NULL OR lease_until<=to_timestamp($1)) AND attempts<10000 ORDER BY next_attempt_at,id FOR UPDATE SKIP LOCKED LIMIT 1) UPDATE auth_apple_revoke_queue q SET lease_id=$2,lease_until=to_timestamp($1)+interval '60 seconds',attempts=attempts+1 FROM candidate c WHERE q.id=c.id RETURNING q.id,q.identity_id,q.encrypted_refresh_token").bind(timestamp(now)?).bind(lease).fetch_optional(&self.pool).await.map_err(database_error)?;
        row.map(|r| {
            Ok(RevokeJob {
                id: r.try_get("id").map_err(database_error)?,
                identity: r.try_get("identity_id").map_err(database_error)?,
                encrypted: r
                    .try_get("encrypted_refresh_token")
                    .map_err(database_error)?,
                lease,
            })
        })
        .transpose()
    }
    async fn finish_revoke(
        &self,
        job: &RevokeJob,
        success: bool,
        now: i64,
    ) -> Result<(), AuthError> {
        let now = timestamp(now)?;
        if success {
            sqlx::query("DELETE FROM auth_apple_revoke_queue WHERE id=$1 AND lease_id=$2")
                .bind(job.id)
                .bind(job.lease)
                .execute(&self.pool)
                .await
                .map_err(database_error)?;
        } else {
            let mut tx = self.pool.begin().await.map_err(database_error)?;
            sqlx::query("DELETE FROM auth_apple_revoke_queue WHERE id=$1 AND lease_id=$2 AND expires_at<=to_timestamp($3)").bind(job.id).bind(job.lease).bind(now).execute(&mut *tx).await.map_err(database_error)?;
            sqlx::query("UPDATE auth_apple_revoke_queue SET lease_id=NULL,lease_until=NULL,next_attempt_at=LEAST(expires_at,to_timestamp($3)+make_interval(secs=>LEAST(3600,power(2,LEAST(attempts,10))*60)::int)) WHERE id=$1 AND lease_id=$2").bind(job.id).bind(job.lease).bind(now).execute(&mut *tx).await.map_err(database_error)?;
            tx.commit().await.map_err(database_error)?;
        }
        Ok(())
    }
    async fn claim_credential(&self, now: i64) -> Result<Option<CredentialJob>, AuthError> {
        let revision = Uuid::new_v4();
        let row=sqlx::query("WITH candidate AS (SELECT identity_id FROM auth_credentials WHERE next_check_at<=to_timestamp($1) AND updated_at<=to_timestamp($1) ORDER BY next_check_at,identity_id FOR UPDATE SKIP LOCKED LIMIT 1) UPDATE auth_credentials c SET next_check_at=to_timestamp($1)+interval '1 day',check_revision=$2 FROM candidate i WHERE c.identity_id=i.identity_id RETURNING c.identity_id,c.encrypted_refresh_token").bind(timestamp(now)?).bind(revision).fetch_optional(&self.pool).await.map_err(database_error)?;
        row.map(|r| {
            Ok(CredentialJob {
                identity: r.try_get("identity_id").map_err(database_error)?,
                encrypted: r
                    .try_get("encrypted_refresh_token")
                    .map_err(database_error)?,
                revision,
            })
        })
        .transpose()
    }
    async fn finish_credential(
        &self,
        job: &CredentialJob,
        status: CredentialCheck,
        now: i64,
    ) -> Result<Option<Uuid>, AuthError> {
        timestamp(now)?;
        if matches!(status, CredentialCheck::Unavailable) {
            return Ok(None);
        }
        let mut tx = self.pool.begin().await.map_err(database_error)?;
        let account = sqlx::query_scalar::<_, Uuid>(
            "SELECT account_id FROM auth_identities WHERE id=$1 AND provider='apple'",
        )
        .bind(job.identity)
        .fetch_optional(&mut *tx)
        .await
        .map_err(database_error)?;
        let Some(account) = account else {
            return Ok(None);
        };
        match lock_account(&mut tx, account).await {
            Ok(_) => {}
            Err(AuthError::Unauthenticated) => return Ok(None),
            Err(e) => return Err(e),
        }
        let row=sqlx::query("SELECT i.subject_digest,i.digest_key_version FROM auth_credentials c JOIN auth_identities i ON i.id=c.identity_id WHERE c.identity_id=$1 AND c.encrypted_refresh_token=$2 AND c.check_revision=$3 FOR UPDATE OF c").bind(job.identity).bind(&job.encrypted).bind(job.revision).fetch_optional(&mut *tx).await.map_err(database_error)?;
        let Some(row) = row else { return Ok(None) };
        let mut affected = None;
        match status {
            CredentialCheck::Revoked => {
                detach_apple(&mut tx, job.identity, account, now).await?;
                affected = Some(account);
            }
            CredentialCheck::Valid {
                digests,
                replacement,
            } => {
                validate_digests(&digests)?;
                let digest: Vec<u8> = row.try_get("subject_digest").map_err(database_error)?;
                let version: i32 = row.try_get("digest_key_version").map_err(database_error)?;
                if !digests
                    .iter()
                    .any(|(v, d)| *v == version as u32 && d.as_slice() == digest)
                {
                    return Err(AuthError::Invalid);
                }
                let bytes = if let Some(refresh) = replacement {
                    if !super::provider::bounded_text(&refresh, 4096) {
                        return Err(AuthError::Invalid);
                    }
                    self.vault.as_ref().ok_or(AuthError::Unavailable)?.seal(
                        job.identity,
                        Provider::Apple,
                        CredentialPurpose::AppleRevoke,
                        refresh.as_bytes(),
                    )?
                } else {
                    job.encrypted.clone()
                };
                sqlx::query("UPDATE auth_credentials SET encrypted_refresh_token=$2,updated_at=to_timestamp($3) WHERE identity_id=$1").bind(job.identity).bind(bytes).bind(timestamp(now)?).execute(&mut *tx).await.map_err(database_error)?;
            }
            CredentialCheck::Unavailable => {}
        }
        let _authority_barrier = affected.map(|account| self.invalidations.account(account));
        tx.commit().await.map_err(database_error)?;
        Ok(affected)
    }
}
async fn detach_apple(
    tx: &mut Transaction<'_, Postgres>,
    identity: Uuid,
    account: Uuid,
    now: i64,
) -> Result<(), AuthError> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_identities WHERE account_id=$1")
        .bind(account)
        .fetch_one(&mut **tx)
        .await
        .map_err(database_error)?;
    // The provider has already invalidated this credential; do not queue it for a redundant revoke.
    sqlx::query("DELETE FROM auth_credentials WHERE identity_id=$1")
        .bind(identity)
        .execute(&mut **tx)
        .await
        .map_err(database_error)?;
    if count <= 1 {
        erase_locked(tx, account, now).await?;
    } else {
        sqlx::query(
            "DELETE FROM auth_identities WHERE id=$1 AND account_id=$2 AND provider='apple'",
        )
        .bind(identity)
        .bind(account)
        .execute(&mut **tx)
        .await
        .map_err(database_error)?;
        revoke_locked(tx, account).await?;
    }
    Ok(())
}
