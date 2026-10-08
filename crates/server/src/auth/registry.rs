use super::*;
use std::future::Future;
use std::sync::Arc;
use zeroize::Zeroizing;

pub trait AuthClock: Send + Sync {
    fn now(&self) -> i64;
}
pub struct SystemAuthClock;
impl AuthClock for SystemAuthClock {
    fn now(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
            .unwrap_or(-1)
    }
}
pub struct UpstreamRequest {
    pub endpoint: &'static str,
    pub form: Vec<(&'static str, Zeroizing<String>)>,
    pub bearer: Option<Zeroizing<String>>,
    pub limit: usize,
}
pub trait OAuthTransport: Send + Sync {
    fn request(
        &self,
        request: UpstreamRequest,
    ) -> impl Future<Output = Result<Zeroizing<Vec<u8>>, AuthError>> + Send;
}
pub struct HttpsOAuthTransport {
    client: reqwest::Client,
}
impl HttpsOAuthTransport {
    pub fn new() -> Result<Self, AuthError> {
        reqwest::Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .map(|client| Self { client })
            .map_err(|_| AuthError::Unavailable)
    }
}
impl OAuthTransport for HttpsOAuthTransport {
    async fn request(&self, request: UpstreamRequest) -> Result<Zeroizing<Vec<u8>>, AuthError> {
        if !matches!(
            request.endpoint,
            "https://oauth2.googleapis.com/token"
                | "https://www.googleapis.com/oauth2/v3/certs"
                | "https://appleid.apple.com/auth/token"
                | "https://appleid.apple.com/auth/keys"
                | "https://appleid.apple.com/auth/revoke"
                | "https://kauth.kakao.com/oauth/token"
                | "https://kauth.kakao.com/.well-known/jwks.json"
                | "https://nid.naver.com/oauth2.0/token"
                | "https://openapi.naver.com/v1/nid/me"
        ) || request.limit > 128 * 1024
            || request.limit == 0
        {
            return Err(AuthError::Invalid);
        }
        let mut builder = if request.form.is_empty() {
            self.client.get(request.endpoint)
        } else {
            let form: Vec<_> = request.form.iter().map(|(k, v)| (*k, v.as_str())).collect();
            self.client.post(request.endpoint).form(&form)
        };
        if let Some(bearer) = &request.bearer {
            builder = builder.bearer_auth(bearer.as_str());
        }
        let refresh = request.endpoint == "https://appleid.apple.com/auth/token"
            && request
                .form
                .iter()
                .any(|(k, v)| *k == "grant_type" && v.as_str() == "refresh_token");
        let response = builder.send().await.map_err(|_| AuthError::Unavailable)?;
        bounded_grant_response(response, request.limit, refresh).await
    }
}
#[cfg(test)]
async fn bounded_response(
    response: reqwest::Response,
    limit: usize,
) -> Result<Zeroizing<Vec<u8>>, AuthError> {
    bounded_grant_response(response, limit, false).await
}
async fn bounded_grant_response(
    mut response: reqwest::Response,
    limit: usize,
    refresh: bool,
) -> Result<Zeroizing<Vec<u8>>, AuthError> {
    let rejected = refresh && response.status() == reqwest::StatusCode::BAD_REQUEST;
    if (response.status() != reqwest::StatusCode::OK && !rejected)
        || response
            .content_length()
            .is_some_and(|len| len > limit as u64)
    {
        return Err(AuthError::Unavailable);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    while let Some(chunk) = response.chunk().await.map_err(|_| AuthError::Unavailable)? {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(AuthError::Unavailable);
        }
        bytes.extend_from_slice(&chunk);
    }
    if rejected {
        #[derive(serde::Deserialize)]
        struct ErrorBody {
            error: String,
        }
        return Err(
            if serde_json::from_slice::<ErrorBody>(&bytes).is_ok_and(|e| e.error == "invalid_grant")
            {
                AuthError::CredentialRevoked
            } else {
                AuthError::Unavailable
            },
        );
    }
    Ok(bytes)
}
#[derive(Default)]
struct JwksCache {
    bytes: Vec<u8>,
    fetched: Option<std::time::Instant>,
    attempted: Option<std::time::Instant>,
    forced: Option<std::time::Instant>,
}
pub struct ProviderRegistry<H> {
    transport: H,
    pub(super) clock: Arc<dyn AuthClock>,
    pub(super) configs: Vec<ProviderConfig>,
    caches: Vec<tokio::sync::Mutex<JwksCache>>,
    pub(super) notification_audience: Option<String>,
}
impl<H: OAuthTransport> ProviderRegistry<H> {
    pub(super) async fn upstream(
        &self,
        request: UpstreamRequest,
    ) -> Result<Zeroizing<Vec<u8>>, AuthError> {
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.transport.request(request),
        )
        .await
        .map_err(|_| AuthError::Unavailable)?
    }
    pub fn new(
        transport: H,
        clock: Arc<dyn AuthClock>,
        configs: Vec<ProviderConfig>,
    ) -> Result<Self, AuthError> {
        for (i, config) in configs.iter().enumerate() {
            config.validate()?;
            if configs[..i].iter().any(|c| c.provider == config.provider) {
                return Err(AuthError::Invalid);
            }
        }
        let caches = configs
            .iter()
            .map(|_| tokio::sync::Mutex::new(JwksCache::default()))
            .collect();
        Ok(Self {
            transport,
            clock,
            configs,
            caches,
            notification_audience: None,
        })
    }
    pub fn with_notification_audience(
        mut self,
        audience: Option<String>,
    ) -> Result<Self, AuthError> {
        if audience
            .as_ref()
            .is_some_and(|a| !super::provider::bounded_text(a, 512))
            || (audience.is_some() && !self.configs.iter().any(|c| c.provider == Provider::Apple))
        {
            return Err(AuthError::Invalid);
        }
        self.notification_audience = audience;
        Ok(self)
    }
    pub(super) async fn keys(&self, index: usize, kid: &str) -> Result<Vec<u8>, AuthError> {
        let mut cache = self.caches[index].lock().await;
        let fresh = cache.fetched.is_some_and(|t| t.elapsed().as_secs() < 300);
        if fresh
            && super::provider::parse_jwks(&cache.bytes)?
                .find(kid)
                .is_some()
        {
            return Ok(cache.bytes.clone());
        }
        if (fresh && cache.forced.is_some_and(|t| t.elapsed().as_secs() < 30))
            || (!fresh && cache.attempted.is_some_and(|t| t.elapsed().as_secs() < 30))
        {
            return Err(if fresh {
                AuthError::Invalid
            } else {
                AuthError::Unavailable
            });
        }
        cache.attempted = Some(std::time::Instant::now());
        if fresh {
            cache.forced = Some(std::time::Instant::now());
        }
        let endpoint = match self.configs[index].provider {
            Provider::Google => "https://www.googleapis.com/oauth2/v3/certs",
            Provider::Apple => "https://appleid.apple.com/auth/keys",
            Provider::Kakao => "https://kauth.kakao.com/.well-known/jwks.json",
            Provider::Naver => return Err(AuthError::Invalid),
        };
        let bytes = self
            .upstream(UpstreamRequest {
                endpoint,
                form: vec![],
                bearer: None,
                limit: 128 * 1024,
            })
            .await?;
        let set = super::provider::parse_jwks(&bytes)?;
        cache.bytes = bytes.to_vec();
        cache.fetched = Some(std::time::Instant::now());
        if set.find(kid).is_none() {
            return Err(AuthError::Invalid);
        }
        Ok(cache.bytes.clone())
    }
}
impl<H: OAuthTransport> OAuthProvider for ProviderRegistry<H> {
    fn available(&self, provider: Provider) -> bool {
        self.configs.iter().any(|c| c.provider == provider)
    }
    fn authorize(&self, provider: Provider, auth: &Authorization) -> Result<String, AuthError> {
        self.configs
            .iter()
            .find(|c| c.provider == provider)
            .ok_or(AuthError::Unavailable)?
            .authorize(auth)
    }
    async fn exchange(
        &self,
        transaction: &AuthTransaction,
        state: &SecretToken,
        code: &str,
        verifier: Option<&[u8]>,
        now: i64,
    ) -> Result<VerifiedIdentity, AuthError> {
        if !super::provider::bounded_text(code, 2048)
            || transaction.state_hash != state.hash()
            || now < transaction.created_at
            || now >= transaction.expires_at
        {
            return Err(AuthError::Invalid);
        }
        let index = self
            .configs
            .iter()
            .position(|c| c.provider == transaction.provider)
            .ok_or(AuthError::Unavailable)?;
        let config = &self.configs[index];
        let secret = if transaction.provider == Provider::Apple {
            config.assertion(now)?
        } else {
            Zeroizing::new(config.client_secret.to_string())
        };
        let z = |value: &str| Zeroizing::new(value.to_string());
        let mut form = vec![
            ("grant_type", z("authorization_code")),
            ("client_id", z(&config.client_id)),
            ("client_secret", secret),
            ("redirect_uri", z(config.callback.as_str())),
            ("code", z(code)),
        ];
        if transaction.provider.uses_pkce() {
            let verifier = std::str::from_utf8(verifier.ok_or(AuthError::Invalid)?)
                .map_err(|_| AuthError::Invalid)?;
            SecretToken::parse(verifier)?;
            form.push(("code_verifier", z(verifier)));
        }
        if transaction.provider == Provider::Naver {
            form.push(("state", state.expose()));
        }
        let endpoint = match transaction.provider {
            Provider::Google => "https://oauth2.googleapis.com/token",
            Provider::Apple => "https://appleid.apple.com/auth/token",
            Provider::Kakao => "https://kauth.kakao.com/oauth/token",
            Provider::Naver => "https://nid.naver.com/oauth2.0/token",
        };
        let bytes = self
            .upstream(UpstreamRequest {
                endpoint,
                form,
                bearer: None,
                limit: 64 * 1024,
            })
            .await?;
        if bytes.len() > 64 * 1024 {
            return Err(AuthError::Unavailable);
        }
        #[derive(serde::Deserialize, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
        struct Tokens {
            access_token: Option<String>,
            token_type: Option<String>,
            id_token: Option<String>,
            refresh_token: Option<String>,
            error: Option<String>,
        }
        let mut tokens: Tokens =
            serde_json::from_slice(&bytes).map_err(|_| AuthError::Unavailable)?;
        if tokens.error.is_some()
            || tokens.access_token.as_ref().is_some_and(|t| {
                !super::provider::bounded_text(t, 8192)
                    || !tokens
                        .token_type
                        .as_deref()
                        .is_some_and(|t| t.eq_ignore_ascii_case("bearer"))
            })
        {
            return Err(AuthError::Invalid);
        }
        if transaction.provider == Provider::Naver {
            let access = tokens.access_token.take().ok_or(AuthError::Invalid)?;
            let bytes = self
                .upstream(UpstreamRequest {
                    endpoint: "https://openapi.naver.com/v1/nid/me",
                    form: vec![],
                    bearer: Some(Zeroizing::new(access)),
                    limit: 64 * 1024,
                })
                .await?;
            if bytes.len() > 64 * 1024 {
                return Err(AuthError::Unavailable);
            }
            #[derive(serde::Deserialize)]
            struct Profile {
                resultcode: String,
                response: Option<Identity>,
            }
            #[derive(serde::Deserialize)]
            struct Identity {
                id: String,
            }
            let profile: Profile =
                serde_json::from_slice(&bytes).map_err(|_| AuthError::Invalid)?;
            if profile.resultcode != "00" {
                return Err(AuthError::Invalid);
            }
            let subject = profile.response.ok_or(AuthError::Invalid)?.id;
            if subject.is_empty()
                || subject.len() > 512
                || subject.chars().any(char::is_control)
                || self.clock.now() >= transaction.expires_at
            {
                return Err(AuthError::Invalid);
            }
            return Ok(VerifiedIdentity {
                subject: Zeroizing::new(subject),
                apple_refresh: None,
            });
        }
        let token = tokens.id_token.as_deref().ok_or(AuthError::Invalid)?;
        let kid = super::provider::token_kid(token)?;
        let keys = self.keys(index, &kid).await?;
        let subject = verify_id_token(
            config,
            &keys,
            token,
            transaction,
            tokens.access_token.as_deref(),
            self.clock.now(),
        )?;
        let apple_refresh = if transaction.provider == Provider::Apple {
            let refresh = tokens.refresh_token.take().ok_or(AuthError::Invalid)?;
            if !super::provider::bounded_text(&refresh, 4096) {
                return Err(AuthError::Invalid);
            }
            Some(Zeroizing::new(refresh))
        } else {
            None
        };
        Ok(VerifiedIdentity {
            subject,
            apple_refresh,
        })
    }
}
#[cfg(test)]
#[path = "../../tests/unit/oauth_transport.rs"]
mod tests;
