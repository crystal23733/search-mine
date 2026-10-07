use super::{Account, AuthError, AuthTransaction, Provider, Session};
use std::future::Future;
use uuid::Uuid;

pub struct LoginWrite {
    pub provider: Provider,
    pub digests: Vec<(u32, [u8; 32])>,
    pub session_hash: [u8; 32],
    pub previous_session: Option<[u8; 32]>,
    pub now: i64,
    pub intent: super::AuthIntent,
    pub bound_session: Option<[u8; 32]>,
    pub apple_refresh: Option<zeroize::Zeroizing<String>>,
}
pub struct LoginRecord {
    pub account: Account,
    pub identity_id: Uuid,
}

pub trait AuthStore: Send + Sync {
    fn insert_transaction(
        &self,
        value: AuthTransaction,
    ) -> impl Future<Output = Result<(), AuthError>> + Send;
    fn consume_transaction(
        &self,
        state: [u8; 32],
        browser: [u8; 32],
        provider: Provider,
        now: i64,
    ) -> impl Future<Output = Result<Option<AuthTransaction>, AuthError>> + Send;
    fn login(
        &self,
        value: LoginWrite,
    ) -> impl Future<Output = Result<LoginRecord, AuthError>> + Send;
    fn session(
        &self,
        hash: [u8; 32],
        now: i64,
    ) -> impl Future<Output = Result<Option<Session>, AuthError>> + Send;
    fn nickname(
        &self,
        account: Uuid,
        value: super::Nickname,
    ) -> impl Future<Output = Result<bool, AuthError>> + Send;
    fn logout(&self, hash: [u8; 32]) -> impl Future<Output = Result<(), AuthError>> + Send;
    fn revoke_account(&self, account: Uuid) -> impl Future<Output = Result<(), AuthError>> + Send;
    fn cleanup(&self, now: i64) -> impl Future<Output = Result<(), AuthError>> + Send;
}
