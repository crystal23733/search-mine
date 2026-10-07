use super::*;
impl PgAuthStore {
    pub(super) async fn authenticate(&self, _value: LoginWrite) -> Result<LoginRecord, AuthError> {
        Err(AuthError::Unavailable)
    }
}
