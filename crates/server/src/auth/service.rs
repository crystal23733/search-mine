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
pub struct AccountAuthorization {
    pub provider: Provider,
    pub intent: AuthIntent,
    pub return_path: ReturnPath,
    pub locale: AuthLocale,
}
pub struct AuthService<S, V, D = DigestKeys> {
    pub store: S,
    vault: V,
    digests: D,
}
impl<S: AuthStore, V: CredentialVault, D: SubjectDigester> AuthService<S, V, D> {
    pub fn new(store: S, vault: V, digests: D) -> Self {
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
        self.start_localized(
            browser,
            provider,
            intent,
            return_path,
            AuthLocale::parse("en")?,
            now,
        )
        .await
    }
    pub async fn start_localized(
        &self,
        browser: &SecretToken,
        provider: Provider,
        intent: AuthIntent,
        return_path: ReturnPath,
        locale: AuthLocale,
        now: i64,
    ) -> Result<Authorization, AuthError> {
        if intent != AuthIntent::Login {
            return Err(AuthError::Invalid);
        }
        self.start_request(
            browser,
            AccountAuthorization {
                provider,
                intent,
                return_path,
                locale,
            },
            None,
            now,
        )
        .await
    }
    pub async fn start_account(
        &self,
        browser: &SecretToken,
        token: &SecretToken,
        request: AccountAuthorization,
        now: i64,
    ) -> Result<Authorization, AuthError> {
        let session = self
            .session(token, now)
            .await?
            .ok_or(AuthError::Unauthenticated)?;
        if request.intent.account() != Some(session.account.id) {
            return Err(AuthError::Invalid);
        }
        if matches!(request.intent, AuthIntent::Link(_)) && !session.fresh(now, RECENT_AUTH_SECONDS)
        {
            return Err(AuthError::ReauthenticationRequired);
        }
        self.start_request(browser, request, Some(token.hash()), now)
            .await
    }
    async fn start_request(
        &self,
        browser: &SecretToken,
        request: AccountAuthorization,
        bound_session_hash: Option<[u8; 32]>,
        now: i64,
    ) -> Result<Authorization, AuthError> {
        let AccountAuthorization {
            provider,
            intent,
            return_path,
            locale,
        } = request;
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
                locale,
                created_at: now,
                expires_at,
                encrypted_verifier,
                bound_session_hash,
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
        let transaction = self
            .store
            .consume_transaction(state.hash(), browser.hash(), provider, now)
            .await?
            .ok_or(AuthError::Invalid)?;
        if !transaction.matches(state, browser, provider, now) {
            return Err(AuthError::Invalid);
        }
        Ok(transaction)
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
        if transaction.intent != AuthIntent::Login || transaction.provider != provider {
            return Err(AuthError::Invalid);
        }
        self.finish_verified(
            transaction,
            VerifiedIdentity {
                subject: zeroize::Zeroizing::new(subject.to_string()),
                apple_refresh: None,
            },
            previous,
            now,
        )
        .await
    }
    pub async fn finish_verified(
        &self,
        transaction: AuthTransaction,
        identity: VerifiedIdentity,
        previous: Option<&SecretToken>,
        now: i64,
    ) -> Result<IssuedSession, AuthError> {
        let provider = transaction.provider;
        if now < transaction.created_at
            || now >= transaction.expires_at
            || now.checked_add(SESSION_SECONDS).is_none()
        {
            return Err(AuthError::Invalid);
        }
        match transaction.intent {
            AuthIntent::Login if transaction.bound_session_hash.is_some() => {
                return Err(AuthError::Invalid);
            }
            AuthIntent::Link(_) | AuthIntent::Reauth(_)
                if previous.map(SecretToken::hash) != transaction.bound_session_hash
                    || transaction.bound_session_hash.is_none() =>
            {
                return Err(AuthError::Invalid);
            }
            _ => {}
        }
        match (provider, identity.apple_refresh.as_ref()) {
            (Provider::Apple, Some(refresh)) if super::provider::bounded_text(refresh, 4096) => {}
            (Provider::Apple, _) | (_, Some(_)) => return Err(AuthError::Invalid),
            _ => {}
        }
        let token = SecretToken::generate()?;
        let result = self
            .store
            .login(LoginWrite {
                provider,
                digests: self.digests.digest(provider, &identity.subject)?,
                session_hash: token.hash(),
                previous_session: previous.map(SecretToken::hash),
                now,
                intent: transaction.intent,
                bound_session: transaction.bound_session_hash,
                apple_refresh: identity.apple_refresh,
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
    pub fn subject_digests(
        &self,
        provider: Provider,
        subject: &str,
    ) -> Result<Vec<(u32, [u8; 32])>, AuthError> {
        self.digests.digest(provider, subject)
    }
}
