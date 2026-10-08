use super::*;
use zeroize::Zeroizing;
impl<H: OAuthTransport> AppleProvider for ProviderRegistry<H> {
    async fn notification(&self, token: &str, now: i64) -> Result<AppleNotification, AuthError> {
        let audience = self
            .notification_audience
            .as_deref()
            .ok_or(AuthError::Unavailable)?;
        let index = self
            .configs
            .iter()
            .position(|c| c.provider == Provider::Apple)
            .ok_or(AuthError::Unavailable)?;
        let keys = self
            .keys(index, &super::provider::token_kid(token)?)
            .await?;
        verify_apple_notification(&keys, token, audience, now)
    }
    async fn revoke_apple(&self, refresh: &str, now: i64) -> Result<(), AuthError> {
        if !super::provider::bounded_text(refresh, 4096) {
            return Err(AuthError::Invalid);
        }
        let config = self
            .configs
            .iter()
            .find(|c| c.provider == Provider::Apple)
            .ok_or(AuthError::Unavailable)?;
        let z = |s: &str| Zeroizing::new(s.to_string());
        self.upstream(UpstreamRequest {
            endpoint: "https://appleid.apple.com/auth/revoke",
            form: vec![
                ("client_id", z(&config.client_id)),
                ("client_secret", config.assertion(now)?),
                ("token", z(refresh)),
                ("token_type_hint", z("refresh_token")),
            ],
            bearer: None,
            limit: 65536,
        })
        .await?;
        Ok(())
    }
    async fn check_apple(
        &self,
        refresh: &str,
        now: i64,
    ) -> Result<AppleCredentialStatus, AuthError> {
        if !super::provider::bounded_text(refresh, 4096) {
            return Err(AuthError::Invalid);
        }
        let index = self
            .configs
            .iter()
            .position(|c| c.provider == Provider::Apple)
            .ok_or(AuthError::Unavailable)?;
        let config = &self.configs[index];
        let z = |s: &str| Zeroizing::new(s.to_string());
        let result = self
            .upstream(UpstreamRequest {
                endpoint: "https://appleid.apple.com/auth/token",
                form: vec![
                    ("grant_type", z("refresh_token")),
                    ("client_id", z(&config.client_id)),
                    ("client_secret", config.assertion(now)?),
                    ("refresh_token", z(refresh)),
                ],
                bearer: None,
                limit: 65536,
            })
            .await;
        let bytes = match result {
            Err(AuthError::CredentialRevoked) => return Ok(AppleCredentialStatus::Revoked),
            result => result?,
        };
        #[derive(serde::Deserialize, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
        struct Tokens {
            id_token: Option<String>,
            refresh_token: Option<String>,
            error: Option<String>,
        }
        let mut tokens: Tokens =
            serde_json::from_slice(&bytes).map_err(|_| AuthError::Unavailable)?;
        if tokens.error.is_some() {
            return Err(AuthError::Unavailable);
        }
        let token = tokens.id_token.as_deref().ok_or(AuthError::Invalid)?;
        let keys = self
            .keys(index, &super::provider::token_kid(token)?)
            .await?;
        let subject =
            verify_apple_refresh_identity(&keys, token, &config.client_id, self.clock.now())?;
        let replacement = tokens.refresh_token.take().map(Zeroizing::new);
        if replacement
            .as_ref()
            .is_some_and(|v| !super::provider::bounded_text(v, 4096))
        {
            return Err(AuthError::Invalid);
        }
        Ok(AppleCredentialStatus::Valid {
            subject,
            replacement,
        })
    }
}
impl<T: OAuthProvider> OAuthProvider for std::sync::Arc<T> {
    fn available(&self, p: Provider) -> bool {
        (**self).available(p)
    }
    fn authorize(&self, p: Provider, a: &Authorization) -> Result<String, AuthError> {
        (**self).authorize(p, a)
    }
    async fn exchange(
        &self,
        t: &AuthTransaction,
        s: &SecretToken,
        c: &str,
        v: Option<&[u8]>,
        n: i64,
    ) -> Result<VerifiedIdentity, AuthError> {
        (**self).exchange(t, s, c, v, n).await
    }
}
impl<T: AppleProvider> AppleProvider for std::sync::Arc<T> {
    async fn notification(&self, t: &str, n: i64) -> Result<AppleNotification, AuthError> {
        (**self).notification(t, n).await
    }
    async fn revoke_apple(&self, r: &str, n: i64) -> Result<(), AuthError> {
        (**self).revoke_apple(r, n).await
    }
    async fn check_apple(&self, r: &str, n: i64) -> Result<AppleCredentialStatus, AuthError> {
        (**self).check_apple(r, n).await
    }
}
