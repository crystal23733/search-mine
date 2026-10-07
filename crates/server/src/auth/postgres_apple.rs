use super::*;
impl AppleMaintenanceStore for PgAuthStore {
    async fn apply_notification(
        &self,
        _jti: [u8; 32],
        _digests: Option<Vec<(u32, [u8; 32])>>,
        _now: i64,
    ) -> Result<Option<uuid::Uuid>, AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn claim_revoke(&self, _now: i64) -> Result<Option<RevokeJob>, AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn finish_revoke(
        &self,
        _job: &RevokeJob,
        _success: bool,
        _now: i64,
    ) -> Result<(), AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn claim_credential(&self, _now: i64) -> Result<Option<CredentialJob>, AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn finish_credential(
        &self,
        _job: &CredentialJob,
        _status: CredentialCheck,
        _now: i64,
    ) -> Result<Option<uuid::Uuid>, AuthError> {
        Err(AuthError::Unavailable)
    }
}
