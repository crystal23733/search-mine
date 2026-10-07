use super::*;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgAuthStore {
    pool: PgPool,
}
impl PgAuthStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
impl AuthStore for PgAuthStore {
    async fn insert_transaction(&self, _value: AuthTransaction) -> Result<(), AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn consume_transaction(
        &self,
        _state: [u8; 32],
        _browser: [u8; 32],
        _provider: Provider,
        _now: i64,
    ) -> Result<Option<AuthTransaction>, AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn login(&self, _value: LoginWrite) -> Result<LoginRecord, AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn session(&self, _hash: [u8; 32], _now: i64) -> Result<Option<Session>, AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn nickname(&self, _account: Uuid, _value: Nickname) -> Result<bool, AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn logout(&self, _hash: [u8; 32]) -> Result<(), AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn revoke_account(&self, _account: Uuid) -> Result<(), AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn cleanup(&self, _now: i64) -> Result<(), AuthError> {
        Err(AuthError::Unavailable)
    }
}
