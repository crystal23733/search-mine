use super::*;
use std::future::Future;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct SessionAuthority {
    pub account: Uuid,
    pub hash: [u8; 32],
    pub now: i64,
}
pub struct LinkedIdentity {
    pub provider: Provider,
    pub linked_at: i64,
}
pub struct AccountExport {
    pub account: Account,
    pub created_at: i64,
    pub last_seen_at: i64,
    pub identities: Vec<LinkedIdentity>,
}
pub struct ErasureResult {
    pub manual_apple_disconnect: bool,
}
// Future account-owned records participate in the same transaction via account FK cascade.
pub trait AccountErasure: AuthStore {
    fn erase_authorized(
        &self,
        authority: SessionAuthority,
    ) -> impl Future<Output = Result<ErasureResult, AuthError>> + Send;
}
pub trait AccountStore: AccountErasure {
    fn identities(
        &self,
        authority: SessionAuthority,
    ) -> impl Future<Output = Result<Vec<LinkedIdentity>, AuthError>> + Send;
    fn export(
        &self,
        authority: SessionAuthority,
    ) -> impl Future<Output = Result<AccountExport, AuthError>> + Send;
    fn unlink(
        &self,
        authority: SessionAuthority,
        provider: Provider,
    ) -> impl Future<Output = Result<ErasureResult, AuthError>> + Send;
}
