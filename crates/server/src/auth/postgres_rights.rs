use super::postgres::{account_row, database_error, timestamp};
use super::postgres_accounts::authorize;
use super::*;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;
impl AccountErasure for PgAuthStore {
    async fn erase_authorized(&self, auth: SessionAuthority) -> Result<ErasureResult, AuthError> {
        let mut tx = self.pool.begin().await.map_err(database_error)?;
        authorize(&mut tx, auth, true).await?;
        let _authority_barrier = self.invalidations.account(auth.account);
        let result = erase_locked(&mut tx, auth.account, auth.now).await?;
        tx.commit().await.map_err(database_error)?;
        Ok(result)
    }
}
impl AccountStore for PgAuthStore {
    async fn identities(&self, auth: SessionAuthority) -> Result<Vec<LinkedIdentity>, AuthError> {
        let mut tx = self.pool.begin().await.map_err(database_error)?;
        authorize(&mut tx, auth, false).await?;
        let result = identities(&mut tx, auth.account).await?;
        tx.commit().await.map_err(database_error)?;
        Ok(result)
    }
    async fn export(&self, auth: SessionAuthority) -> Result<AccountExport, AuthError> {
        let mut tx = self.pool.begin().await.map_err(database_error)?;
        let row = authorize(&mut tx, auth, true).await?;
        let result = AccountExport {
            account: account_row(&row)?,
            created_at: row.try_get("created_at").map_err(database_error)?,
            last_seen_at: row.try_get("last_seen_at").map_err(database_error)?,
            identities: identities(&mut tx, auth.account).await?,
        };
        tx.commit().await.map_err(database_error)?;
        Ok(result)
    }
    async fn unlink(
        &self,
        auth: SessionAuthority,
        provider: Provider,
    ) -> Result<ErasureResult, AuthError> {
        let mut tx = self.pool.begin().await.map_err(database_error)?;
        authorize(&mut tx, auth, true).await?;
        let linked = identities(&mut tx, auth.account).await?;
        if !linked.iter().any(|i| i.provider == provider) {
            return Err(AuthError::Invalid);
        }
        if linked.len() <= 1 {
            return Err(AuthError::Conflict);
        }
        let manual = if provider == Provider::Apple {
            enqueue_apple(&mut tx, auth.account, auth.now).await?
        } else {
            false
        };
        sqlx::query("DELETE FROM auth_identities WHERE account_id=$1 AND provider=$2")
            .bind(auth.account)
            .bind(provider.as_str())
            .execute(&mut *tx)
            .await
            .map_err(database_error)?;
        revoke_locked(&mut tx, auth.account).await?;
        let _authority_barrier = self.invalidations.account(auth.account);
        tx.commit().await.map_err(database_error)?;
        Ok(ErasureResult {
            manual_apple_disconnect: manual,
        })
    }
}
async fn identities(
    tx: &mut Transaction<'_, Postgres>,
    account: Uuid,
) -> Result<Vec<LinkedIdentity>, AuthError> {
    let rows=sqlx::query("SELECT provider,EXTRACT(EPOCH FROM linked_at)::bigint AS linked_at FROM auth_identities WHERE account_id=$1 ORDER BY provider").bind(account).fetch_all(&mut **tx).await.map_err(database_error)?;
    rows.iter()
        .map(|row| {
            Ok(LinkedIdentity {
                provider: Provider::parse(
                    &row.try_get::<String, _>("provider")
                        .map_err(database_error)?,
                )?,
                linked_at: row.try_get("linked_at").map_err(database_error)?,
            })
        })
        .collect()
}
pub(super) async fn enqueue_apple(
    tx: &mut Transaction<'_, Postgres>,
    account: Uuid,
    now: i64,
) -> Result<bool, AuthError> {
    let row=sqlx::query("SELECT i.id,c.encrypted_refresh_token FROM auth_identities i LEFT JOIN auth_credentials c ON c.identity_id=i.id WHERE i.account_id=$1 AND i.provider='apple'").bind(account).fetch_optional(&mut **tx).await.map_err(database_error)?;
    if let Some(row) = row {
        let bytes: Option<Vec<u8>> = row
            .try_get("encrypted_refresh_token")
            .map_err(database_error)?;
        let Some(bytes) = bytes else {
            return Ok(true);
        };
        let id: Uuid = row.try_get("id").map_err(database_error)?;
        sqlx::query("INSERT INTO auth_apple_revoke_queue(id,identity_id,encrypted_refresh_token,created_at,expires_at,next_attempt_at) VALUES($1,$2,$3,to_timestamp($4),to_timestamp($4)+interval '24 hours',to_timestamp($4)) ON CONFLICT(identity_id) DO NOTHING").bind(Uuid::new_v4()).bind(id).bind(bytes).bind(timestamp(now)?).execute(&mut **tx).await.map_err(database_error)?;
    }
    Ok(false)
}
pub(super) async fn erase_locked(
    tx: &mut Transaction<'_, Postgres>,
    account: Uuid,
    now: i64,
) -> Result<ErasureResult, AuthError> {
    let manual = enqueue_apple(tx, account, now).await?;
    sqlx::query("INSERT INTO auth_deletion_tombstones(account_id,deleted_at,expires_at) VALUES($1,to_timestamp($2),to_timestamp($2)+interval '28 days') ON CONFLICT(account_id) DO NOTHING").bind(account).bind(timestamp(now)?).execute(&mut **tx).await.map_err(database_error)?;
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(account)
        .execute(&mut **tx)
        .await
        .map_err(database_error)?;
    Ok(ErasureResult {
        manual_apple_disconnect: manual,
    })
}
pub(super) async fn revoke_locked(
    tx: &mut Transaction<'_, Postgres>,
    account: Uuid,
) -> Result<(), AuthError> {
    sqlx::query("DELETE FROM auth_sessions WHERE account_id=$1")
        .bind(account)
        .execute(&mut **tx)
        .await
        .map_err(database_error)?;
    sqlx::query("DELETE FROM auth_transactions WHERE account_id=$1")
        .bind(account)
        .execute(&mut **tx)
        .await
        .map_err(database_error)?;
    Ok(())
}
