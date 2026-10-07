use super::*;
use uuid::Uuid;

pub struct Authorization {
    pub id: Uuid,
    pub state: SecretToken,
    pub nonce: SecretToken,
    pub verifier: Option<SecretToken>,
}
pub struct IssuedSession {
    pub account: Account,
    pub identity_id: Uuid,
    pub token: SecretToken,
}
pub struct AuthService<S, V> {
    pub store: S,
    vault: V,
    digests: DigestKeys,
}
impl<S: AuthStore, V: CredentialVault> AuthService<S, V> {
    pub fn new(store: S, vault: V, digests: DigestKeys) -> Self {
        Self {
            store,
            vault,
            digests,
        }
    }
    pub async fn start(
        &self,
        browser: &SecretToken,
        provider: Provider,
        intent: AuthIntent,
        return_path: ReturnPath,
        now: i64,
    ) -> Result<Authorization, AuthError> {
        let expires_at = now
            .checked_add(TRANSACTION_SECONDS)
            .filter(|_| now >= 0)
            .ok_or(AuthError::Invalid)?;
        if intent.account().is_some_and(|id| id.is_nil()) {
            return Err(AuthError::Invalid);
        }
        let auth = Authorization {
            id: Uuid::new_v4(),
            state: SecretToken::generate()?,
            nonce: SecretToken::generate()?,
            verifier: provider
                .uses_pkce()
                .then(SecretToken::generate)
                .transpose()?,
        };
        let encrypted_verifier = auth
            .verifier
            .as_ref()
            .map(|v| {
                self.vault.seal(
                    auth.id,
                    provider,
                    CredentialPurpose::Pkce,
                    v.expose().as_bytes(),
                )
            })
            .transpose()?;
        self.store
            .insert_transaction(AuthTransaction {
                id: auth.id,
                state_hash: auth.state.hash(),
                browser_hash: browser.hash(),
                nonce_hash: auth.nonce.hash(),
                provider,
                intent,
                return_path,
                created_at: now,
                expires_at,
                encrypted_verifier,
            })
            .await?;
        Ok(auth)
    }
    pub async fn consume(
        &self,
        state: &SecretToken,
        browser: &SecretToken,
        provider: Provider,
        now: i64,
    ) -> Result<AuthTransaction, AuthError> {
        self.store
            .consume_transaction(state.hash(), browser.hash(), provider, now)
            .await?
            .ok_or(AuthError::Invalid)
    }
    pub fn verifier(
        &self,
        transaction: &AuthTransaction,
    ) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, AuthError> {
        transaction
            .encrypted_verifier
            .as_ref()
            .map(|value| {
                self.vault.open(
                    transaction.id,
                    transaction.provider,
                    CredentialPurpose::Pkce,
                    value,
                )
            })
            .transpose()
    }
    pub async fn finish_login(
        &self,
        transaction: AuthTransaction,
        provider: Provider,
        subject: &str,
        previous: Option<&SecretToken>,
        now: i64,
    ) -> Result<IssuedSession, AuthError> {
        if transaction.intent != AuthIntent::Login
            || transaction.provider != provider
            || now < transaction.created_at
            || now >= transaction.expires_at
            || now.checked_add(SESSION_SECONDS).is_none()
        {
            return Err(AuthError::Invalid);
        }
        let token = SecretToken::generate()?;
        let result = self
            .store
            .login(LoginWrite {
                provider,
                digests: self.digests.digest(provider, subject)?,
                session_hash: token.hash(),
                previous_session: previous.map(SecretToken::hash),
                now,
            })
            .await?;
        Ok(IssuedSession {
            account: result.account,
            identity_id: result.identity_id,
            token,
        })
    }
    pub async fn session(
        &self,
        token: &SecretToken,
        now: i64,
    ) -> Result<Option<Session>, AuthError> {
        self.store.session(token.hash(), now).await
    }
}
