use super::postgres::{account_row, database_error, timestamp};
use super::*;
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};
use uuid::Uuid;
impl PgAuthStore {
    pub(super) async fn authenticate(&self, value: LoginWrite) -> Result<LoginRecord, AuthError> {
        let now = timestamp(value.now)?;
        let expiry = timestamp(
            value
                .now
                .checked_add(SESSION_SECONDS)
                .ok_or(AuthError::Invalid)?,
        )?;
        validate_digests(&value.digests)?;
        match (value.intent, value.bound_session) {
            (AuthIntent::Login, None) => {}
            (AuthIntent::Link(id) | AuthIntent::Reauth(id), Some(hash))
                if !id.is_nil() && value.previous_session == Some(hash) => {}
            _ => return Err(AuthError::Invalid),
        }
        match (value.provider, value.apple_refresh.as_ref()) {
            (Provider::Apple, Some(v)) if super::provider::bounded_text(v, 4096) => {}
            (Provider::Apple, _) | (_, Some(_)) => return Err(AuthError::Invalid),
            _ => {}
        }
        let mut tx = self.pool.begin().await.map_err(database_error)?;
        lock_subject(&mut tx, &value.digests).await?;
        let existing = find_identity(&mut tx, value.provider, &value.digests).await?;
        let account_id = if let Some(account) = value.intent.account() {
            authorize(
                &mut tx,
                SessionAuthority {
                    account,
                    hash: value.bound_session.ok_or(AuthError::Invalid)?,
                    now: value.now,
                },
                matches!(value.intent, AuthIntent::Link(_)),
            )
            .await?;
            if existing.is_some_and(|(_, owner)| owner != account) {
                return Err(AuthError::Conflict);
            }
            if matches!(value.intent, AuthIntent::Reauth(_)) && existing.is_none() {
                return Err(AuthError::Invalid);
            }
            account
        } else if let Some((_, account)) = existing {
            lock_account(&mut tx, account).await?;
            account
        } else {
            let account = Uuid::new_v4();
            sqlx::query("INSERT INTO auth_accounts(id,created_at,last_seen_at) VALUES($1,to_timestamp($2),to_timestamp($2))").bind(account).bind(now).execute(&mut *tx).await.map_err(database_error)?;
            account
        };
        // Unlink can remove an identity while this login waits for the account lock.
        // Reconcile the earlier subject lookup with the now-locked account before issuing a session.
        if let Some(expected) = existing
            && find_identity(&mut tx, value.provider, &value.digests).await? != Some(expected)
        {
            return Err(AuthError::Unauthenticated);
        }
        let (version, digest) = value.digests[0];
        let identity_id = if let Some((id, _)) = existing {
            sqlx::query(
                "UPDATE auth_identities SET subject_digest=$2,digest_key_version=$3 WHERE id=$1",
            )
            .bind(id)
            .bind(digest.as_slice())
            .bind(version as i32)
            .execute(&mut *tx)
            .await
            .map_err(database_error)?;
            id
        } else {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO auth_identities(id,account_id,provider,issuer,subject_digest,digest_key_version,linked_at) VALUES($1,$2,$3,$4,$5,$6,to_timestamp($7))").bind(id).bind(account_id).bind(value.provider.as_str()).bind(value.provider.issuer()).bind(digest.as_slice()).bind(version as i32).bind(now).execute(&mut *tx).await.map_err(database_error)?;
            id
        };
        let changed=sqlx::query("UPDATE auth_accounts SET last_seen_at=GREATEST(last_seen_at,to_timestamp($2)) WHERE id=$1 AND created_at<=to_timestamp($2)").bind(account_id).bind(now).execute(&mut *tx).await.map_err(database_error)?;
        if changed.rows_affected() != 1 {
            return Err(AuthError::Invalid);
        }
        if let Some(refresh) = value.apple_refresh {
            let sealed = self.vault.as_ref().ok_or(AuthError::Unavailable)?.seal(
                identity_id,
                Provider::Apple,
                CredentialPurpose::AppleRevoke,
                refresh.as_bytes(),
            )?;
            sqlx::query("INSERT INTO auth_credentials(identity_id,encrypted_refresh_token,updated_at,next_check_at) VALUES($1,$2,to_timestamp($3),to_timestamp($3)+interval '1 day') ON CONFLICT(identity_id) DO UPDATE SET encrypted_refresh_token=EXCLUDED.encrypted_refresh_token,updated_at=EXCLUDED.updated_at,next_check_at=EXCLUDED.next_check_at,check_revision=gen_random_uuid()").bind(identity_id).bind(sealed).bind(now).execute(&mut *tx).await.map_err(database_error)?;
        }
        if let Some(previous) = value.previous_session {
            sqlx::query("DELETE FROM auth_sessions WHERE token_hash=$1")
                .bind(previous.as_slice())
                .execute(&mut *tx)
                .await
                .map_err(database_error)?;
        }
        sqlx::query("INSERT INTO auth_sessions(id,account_id,token_hash,created_at,authenticated_at,expires_at) VALUES($1,$2,$3,to_timestamp($4),to_timestamp($4),to_timestamp($5))").bind(Uuid::new_v4()).bind(account_id).bind(value.session_hash.as_slice()).bind(now).bind(expiry).execute(&mut *tx).await.map_err(database_error)?;
        let row = sqlx::query("SELECT id,nickname FROM auth_accounts WHERE id=$1")
            .bind(account_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(database_error)?;
        let account = account_row(&row)?;
        let _authority_barrier = value
            .previous_session
            .map(|hash| self.invalidations.session(hash));
        tx.commit().await.map_err(database_error)?;
        Ok(LoginRecord {
            account,
            identity_id,
        })
    }
}
pub(super) fn validate_digests(digests: &[(u32, [u8; 32])]) -> Result<(), AuthError> {
    if digests.is_empty()
        || digests.len() > 4
        || digests.iter().any(|(v, _)| *v == 0 || *v > i32::MAX as u32)
    {
        Err(AuthError::Invalid)
    } else {
        Ok(())
    }
}
async fn lock_subject(
    tx: &mut Transaction<'_, Postgres>,
    digests: &[(u32, [u8; 32])],
) -> Result<(), AuthError> {
    let lock = i64::from_be_bytes(
        digests[0].1[..8]
            .try_into()
            .map_err(|_| AuthError::Invalid)?,
    );
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(lock)
        .execute(&mut **tx)
        .await
        .map_err(database_error)?;
    Ok(())
}
pub(super) async fn find_identity(
    tx: &mut Transaction<'_, Postgres>,
    provider: Provider,
    digests: &[(u32, [u8; 32])],
) -> Result<Option<(Uuid, Uuid)>, AuthError> {
    validate_digests(digests)?;
    let mut found = None;
    for (version, digest) in digests {
        let row=sqlx::query("SELECT id,account_id FROM auth_identities WHERE provider=$1 AND issuer=$2 AND digest_key_version=$3 AND subject_digest=$4").bind(provider.as_str()).bind(provider.issuer()).bind(*version as i32).bind(digest.as_slice()).fetch_optional(&mut **tx).await.map_err(database_error)?;
        if let Some(row) = row {
            let pair = (
                row.try_get("id").map_err(database_error)?,
                row.try_get("account_id").map_err(database_error)?,
            );
            if found.is_some_and(|saved| saved != pair) {
                return Err(AuthError::Conflict);
            }
            found = Some(pair);
        }
    }
    Ok(found)
}
pub(super) async fn lock_account(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<PgRow, AuthError> {
    sqlx::query("SELECT id,nickname,EXTRACT(EPOCH FROM created_at)::bigint AS created_at,EXTRACT(EPOCH FROM last_seen_at)::bigint AS last_seen_at FROM auth_accounts WHERE id=$1 FOR UPDATE").bind(id).fetch_optional(&mut **tx).await.map_err(database_error)?.ok_or(AuthError::Unauthenticated)
}
pub(super) async fn authorize(
    tx: &mut Transaction<'_, Postgres>,
    auth: SessionAuthority,
    fresh: bool,
) -> Result<PgRow, AuthError> {
    let now = timestamp(auth.now)?;
    let row = lock_account(tx, auth.account).await?;
    let at=sqlx::query_scalar::<_,i64>("SELECT EXTRACT(EPOCH FROM authenticated_at)::bigint FROM auth_sessions WHERE account_id=$1 AND token_hash=$2 AND created_at<=to_timestamp($3) AND expires_at>to_timestamp($3) FOR UPDATE").bind(auth.account).bind(auth.hash.as_slice()).bind(now).fetch_optional(&mut **tx).await.map_err(database_error)?.ok_or(AuthError::Unauthenticated)?;
    if at > auth.now || (fresh && auth.now.saturating_sub(at) >= RECENT_AUTH_SECONDS) {
        return Err(AuthError::ReauthenticationRequired);
    }
    Ok(row)
}
