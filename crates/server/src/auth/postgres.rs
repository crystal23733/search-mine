use super::*;
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgAuthStore {
    pub(super) pool: PgPool,
    pub(super) vault: Option<std::sync::Arc<dyn CredentialVault>>,
    pub(super) invalidations: std::sync::Arc<dyn SessionInvalidator>,
}
impl PgAuthStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            vault: None,
            invalidations: std::sync::Arc::new(NoSessionInvalidator),
        }
    }
    pub fn with_vault(pool: PgPool, vault: std::sync::Arc<dyn CredentialVault>) -> Self {
        Self {
            pool,
            vault: Some(vault),
            invalidations: std::sync::Arc::new(NoSessionInvalidator),
        }
    }
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
    pub fn with_invalidations(
        mut self,
        invalidations: std::sync::Arc<dyn SessionInvalidator>,
    ) -> Self {
        self.invalidations = invalidations;
        self
    }
}
impl AuthStore for PgAuthStore {
    async fn insert_transaction(&self, value: AuthTransaction) -> Result<(), AuthError> {
        let now = timestamp(value.created_at)?;
        let expiry = timestamp(value.expires_at)?;
        sqlx::query("INSERT INTO auth_transactions(id,state_hash,browser_hash,nonce_hash,provider,intent,account_id,return_path,created_at,expires_at,encrypted_verifier,locale,bound_session_hash) VALUES($1,$2,$3,$4,$5,$6,$7,$8,to_timestamp($9),to_timestamp($10),$11,$12,$13)")
            .bind(value.id).bind(value.state_hash.as_slice()).bind(value.browser_hash.as_slice()).bind(value.nonce_hash.as_slice())
            .bind(value.provider.as_str()).bind(value.intent.kind()).bind(value.intent.account()).bind(value.return_path.as_str())
            .bind(now).bind(expiry).bind(value.encrypted_verifier).bind(value.locale.as_str()).bind(value.bound_session_hash.map(|h|h.to_vec())).execute(&self.pool).await.map_err(database_error)?;
        Ok(())
    }
    async fn consume_transaction(
        &self,
        state: [u8; 32],
        browser: [u8; 32],
        provider: Provider,
        now: i64,
    ) -> Result<Option<AuthTransaction>, AuthError> {
        let row=sqlx::query("DELETE FROM auth_transactions WHERE state_hash=$1 AND browser_hash=$2 AND provider=$3 AND created_at<=to_timestamp($4) AND expires_at>to_timestamp($4) RETURNING id,state_hash,browser_hash,nonce_hash,provider,intent,account_id,return_path,EXTRACT(EPOCH FROM created_at)::bigint AS created_at,EXTRACT(EPOCH FROM expires_at)::bigint AS expires_at,encrypted_verifier,locale,bound_session_hash")
            .bind(state.as_slice()).bind(browser.as_slice()).bind(provider.as_str()).bind(timestamp(now)?).fetch_optional(&self.pool).await.map_err(database_error)?;
        row.map(transaction_row).transpose()
    }
    async fn login(&self, value: LoginWrite) -> Result<LoginRecord, AuthError> {
        self.authenticate(value).await
    }
    async fn session(&self, hash: [u8; 32], now: i64) -> Result<Option<Session>, AuthError> {
        let row=sqlx::query("SELECT s.id AS session_id,a.id,a.nickname,EXTRACT(EPOCH FROM s.created_at)::bigint AS created_at,EXTRACT(EPOCH FROM s.expires_at)::bigint AS expires_at,EXTRACT(EPOCH FROM s.authenticated_at)::bigint AS authenticated_at FROM auth_sessions s JOIN auth_accounts a ON a.id=s.account_id WHERE s.token_hash=$1 AND s.created_at<=to_timestamp($2) AND s.expires_at>to_timestamp($2)")
            .bind(hash.as_slice()).bind(timestamp(now)?).fetch_optional(&self.pool).await.map_err(database_error)?;
        row.map(|row| {
            Ok(Session {
                id: row.try_get("session_id").map_err(database_error)?,
                account: account_row(&row)?,
                created_at: row.try_get("created_at").map_err(database_error)?,
                expires_at: row.try_get("expires_at").map_err(database_error)?,
                authenticated_at: row.try_get("authenticated_at").map_err(database_error)?,
            })
        })
        .transpose()
    }
    async fn nickname(&self, account: Uuid, value: Nickname) -> Result<bool, AuthError> {
        Ok(
            sqlx::query("UPDATE auth_accounts SET nickname=$2 WHERE id=$1")
                .bind(account)
                .bind(value.as_str())
                .execute(&self.pool)
                .await
                .map_err(database_error)?
                .rows_affected()
                == 1,
        )
    }
    async fn logout(&self, hash: [u8; 32]) -> Result<(), AuthError> {
        sqlx::query("DELETE FROM auth_sessions WHERE token_hash=$1")
            .bind(hash.as_slice())
            .execute(&self.pool)
            .await
            .map_err(database_error)?;
        Ok(())
    }
    async fn revoke_account(&self, account: Uuid) -> Result<(), AuthError> {
        let mut tx = self.pool.begin().await.map_err(database_error)?;
        sqlx::query("DELETE FROM auth_sessions WHERE account_id=$1")
            .bind(account)
            .execute(&mut *tx)
            .await
            .map_err(database_error)?;
        sqlx::query("DELETE FROM auth_transactions WHERE account_id=$1")
            .bind(account)
            .execute(&mut *tx)
            .await
            .map_err(database_error)?;
        tx.commit().await.map_err(database_error)?;
        Ok(())
    }
    async fn cleanup(&self, now: i64) -> Result<(), AuthError> {
        let now = timestamp(now)?;
        let mut tx = self.pool.begin().await.map_err(database_error)?;
        sqlx::query("DELETE FROM auth_transactions WHERE expires_at<=to_timestamp($1)")
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(database_error)?;
        sqlx::query("DELETE FROM auth_sessions WHERE expires_at<=to_timestamp($1)")
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(database_error)?;
        for query in [
            "DELETE FROM auth_deletion_tombstones WHERE expires_at<=to_timestamp($1)",
            "DELETE FROM auth_apple_notification_receipts WHERE expires_at<=to_timestamp($1)",
            "DELETE FROM auth_apple_revoke_queue WHERE expires_at<=to_timestamp($1)",
        ] {
            sqlx::query(query)
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(database_error)?;
        }
        tx.commit().await.map_err(database_error)?;
        Ok(())
    }
}

pub(super) fn database_error(error: sqlx::Error) -> AuthError {
    if error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation())
    {
        AuthError::Conflict
    } else {
        AuthError::Unavailable
    }
}
pub(super) fn timestamp(now: i64) -> Result<f64, AuthError> {
    if (0..=253_402_300_799).contains(&now) {
        Ok(now as f64)
    } else {
        Err(AuthError::Invalid)
    }
}
pub(super) fn account_row(row: &PgRow) -> Result<Account, AuthError> {
    let nickname: Option<String> = row.try_get("nickname").map_err(database_error)?;
    Ok(Account {
        id: row.try_get("id").map_err(database_error)?,
        nickname: nickname.as_deref().map(Nickname::parse).transpose()?,
    })
}
fn transaction_row(row: PgRow) -> Result<AuthTransaction, AuthError> {
    let provider: String = row.try_get("provider").map_err(database_error)?;
    let intent: String = row.try_get("intent").map_err(database_error)?;
    let path: String = row.try_get("return_path").map_err(database_error)?;
    let digest = |name| -> Result<[u8; 32], AuthError> {
        let value: Vec<u8> = row.try_get(name).map_err(database_error)?;
        value.try_into().map_err(|_| AuthError::Unavailable)
    };
    Ok(AuthTransaction {
        id: row.try_get("id").map_err(database_error)?,
        state_hash: digest("state_hash")?,
        browser_hash: digest("browser_hash")?,
        nonce_hash: digest("nonce_hash")?,
        provider: Provider::parse(&provider)?,
        intent: AuthIntent::restore(&intent, row.try_get("account_id").map_err(database_error)?)?,
        return_path: ReturnPath::parse(&path)?,
        locale: AuthLocale::parse(&row.try_get::<String, _>("locale").map_err(database_error)?)?,
        created_at: row.try_get("created_at").map_err(database_error)?,
        expires_at: row.try_get("expires_at").map_err(database_error)?,
        encrypted_verifier: row.try_get("encrypted_verifier").map_err(database_error)?,
        bound_session_hash: row
            .try_get::<Option<Vec<u8>>, _>("bound_session_hash")
            .map_err(database_error)?
            .map(|v| v.try_into().map_err(|_| AuthError::Unavailable))
            .transpose()?,
    })
}
