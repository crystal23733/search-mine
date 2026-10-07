use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation,
    jwk::{AlgorithmParameters, JwkSet, KeyAlgorithm, KeyOperations, PublicKeyUse},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::future::Future;
use zeroize::Zeroizing;

pub struct ProviderConfig {
    pub provider: Provider,
    pub client_id: String,
    pub client_secret: Zeroizing<String>,
    pub callback: url::Url,
    pub apple: Option<AppleSigning>,
}
pub struct AppleSigning {
    pub team_id: String,
    pub key_id: String,
    pub private_key: Zeroizing<Vec<u8>>,
}
pub struct VerifiedIdentity {
    pub subject: Zeroizing<String>,
    pub apple_refresh: Option<Zeroizing<String>>,
}
pub trait OAuthProvider: Send + Sync {
    fn available(&self, provider: Provider) -> bool;
    fn authorize(&self, provider: Provider, auth: &Authorization) -> Result<String, AuthError>;
    fn exchange(
        &self,
        transaction: &AuthTransaction,
        state: &SecretToken,
        code: &str,
        verifier: Option<&[u8]>,
        now: i64,
    ) -> impl Future<Output = Result<VerifiedIdentity, AuthError>> + Send;
}
impl ProviderConfig {
    pub fn validate(&self) -> Result<(), AuthError> {
        let callback = &self.callback;
        if !bounded_text(&self.client_id, 512)
            || callback.scheme() != "https"
            || !matches!(callback.host(), Some(url::Host::Domain(_)))
            || !callback.username().is_empty()
            || callback.password().is_some()
            || callback.query().is_some()
            || callback.fragment().is_some()
            || callback.path() != format!("/api/v1/auth/{}/callback", self.provider.as_str())
        {
            return Err(AuthError::Invalid);
        }
        if self.provider == Provider::Apple {
            let apple = self.apple.as_ref().ok_or(AuthError::Invalid)?;
            if ![&apple.team_id, &apple.key_id]
                .iter()
                .all(|s| s.len() == 10 && s.bytes().all(|b| b.is_ascii_alphanumeric()))
                || apple.private_key.len() > 8192
                || EncodingKey::from_ec_pem(&apple.private_key).is_err()
            {
                return Err(AuthError::Invalid);
            }
        } else if !bounded_text(&self.client_secret, 4096) || self.apple.is_some() {
            return Err(AuthError::Invalid);
        }
        Ok(())
    }
    pub fn authorize(&self, auth: &Authorization) -> Result<String, AuthError> {
        self.validate()?;
        let endpoint = match self.provider {
            Provider::Google => "https://accounts.google.com/o/oauth2/v2/auth",
            Provider::Apple => "https://appleid.apple.com/auth/authorize",
            Provider::Kakao => "https://kauth.kakao.com/oauth/authorize",
            Provider::Naver => "https://nid.naver.com/oauth2.0/authorize",
        };
        let mut url = url::Url::parse(endpoint).map_err(|_| AuthError::Unavailable)?;
        let mut pairs = url.query_pairs_mut();
        pairs
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", self.callback.as_str())
            .append_pair("response_type", "code")
            .append_pair("state", &auth.state.expose());
        if self.provider != Provider::Naver {
            pairs.append_pair("nonce", &auth.nonce.expose());
        }
        if self.provider.uses_pkce() {
            let verifier = auth.verifier.as_ref().ok_or(AuthError::Invalid)?;
            pairs
                .append_pair("scope", "openid")
                .append_pair("code_challenge_method", "S256")
                .append_pair("code_challenge", &verifier.pkce_challenge());
        }
        if self.provider == Provider::Apple {
            pairs.append_pair("response_mode", "query");
        }
        drop(pairs);
        Ok(url.into())
    }
    pub fn assertion(&self, now: i64) -> Result<Zeroizing<String>, AuthError> {
        self.validate()?;
        let apple = self.apple.as_ref().ok_or(AuthError::Invalid)?;
        let exp = now
            .checked_add(300)
            .filter(|_| now >= 0)
            .ok_or(AuthError::Invalid)?;
        #[derive(Serialize)]
        struct Claims<'a> {
            iss: &'a str,
            sub: &'a str,
            aud: &'a str,
            iat: i64,
            exp: i64,
        }
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(apple.key_id.clone());
        let key = EncodingKey::from_ec_pem(&apple.private_key).map_err(|_| AuthError::Invalid)?;
        jsonwebtoken::encode(
            &header,
            &Claims {
                iss: &apple.team_id,
                sub: &self.client_id,
                aud: "https://appleid.apple.com",
                iat: now,
                exp,
            },
            &key,
        )
        .map(Zeroizing::new)
        .map_err(|_| AuthError::Unavailable)
    }
}
pub(crate) fn bounded_text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| b.is_ascii_graphic())
}
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Audience {
    One(String),
    Many(Vec<String>),
}
impl Audience {
    pub(super) fn matches(&self, client: &str) -> bool {
        match self {
            Self::One(a) => a == client,
            Self::Many(a) => a.len() == 1 && a[0] == client,
        }
    }
}
#[derive(Deserialize)]
struct IdClaims {
    sub: String,
    aud: Audience,
    exp: i64,
    iat: i64,
    nonce: String,
    azp: Option<String>,
    nbf: Option<i64>,
    at_hash: Option<String>,
}
pub(crate) fn token_kid(token: &str) -> Result<String, AuthError> {
    if !bounded_text(token, 16 * 1024) {
        return Err(AuthError::Invalid);
    }
    let part = token.split('.').next().ok_or(AuthError::Invalid)?;
    if part.len() > 4096 {
        return Err(AuthError::Invalid);
    }
    let raw = URL_SAFE_NO_PAD
        .decode(part)
        .map_err(|_| AuthError::Invalid)?;
    let header: serde_json::Value = serde_json::from_slice(&raw).map_err(|_| AuthError::Invalid)?;
    if header.get("crit").is_some()
        || header.get("jku").is_some()
        || header.get("x5u").is_some()
        || header
            .get("typ")
            .is_some_and(|v| v != "JWT" && v != "application/jwt")
    {
        return Err(AuthError::Invalid);
    }
    let header = jsonwebtoken::decode_header(token).map_err(|_| AuthError::Invalid)?;
    if header.alg != Algorithm::RS256 {
        return Err(AuthError::Invalid);
    }
    header
        .kid
        .filter(|s| bounded_text(s, 256))
        .ok_or(AuthError::Invalid)
}
pub(crate) fn parse_jwks(bytes: &[u8]) -> Result<JwkSet, AuthError> {
    if bytes.len() > 128 * 1024 {
        return Err(AuthError::Unavailable);
    }
    let set: JwkSet = serde_json::from_slice(bytes).map_err(|_| AuthError::Unavailable)?;
    if set.keys.is_empty() || set.keys.len() > 64 {
        return Err(AuthError::Unavailable);
    }
    let mut ids = std::collections::HashSet::new();
    for key in &set.keys {
        let id = key.common.key_id.as_deref().ok_or(AuthError::Unavailable)?;
        if !bounded_text(id, 256) || !ids.insert(id) {
            return Err(AuthError::Unavailable);
        }
    }
    Ok(set)
}
pub fn verify_id_token(
    config: &ProviderConfig,
    jwks: &[u8],
    token: &str,
    transaction: &AuthTransaction,
    access_token: Option<&str>,
    now: i64,
) -> Result<Zeroizing<String>, AuthError> {
    if config.provider == Provider::Naver
        || transaction.provider != config.provider
        || now < transaction.created_at
        || now >= transaction.expires_at
    {
        return Err(AuthError::Invalid);
    }
    let issuers = if config.provider == Provider::Google {
        vec![config.provider.issuer(), "accounts.google.com"]
    } else {
        vec![config.provider.issuer()]
    };
    let claims: IdClaims = decode_signed(
        jwks,
        token,
        &config.client_id,
        &issuers,
        &["exp", "iss", "aud", "sub"],
    )?;
    if !claims.aud.matches(&config.client_id)
        || claims.azp.as_ref().is_some_and(|a| a != &config.client_id)
        || claims.exp <= now
        || claims.iat < transaction.created_at.saturating_sub(30)
        || claims.iat > now.saturating_add(30)
        || claims.exp <= claims.iat
        || claims.nbf.is_some_and(|n| n > now)
        || claims.sub.is_empty()
        || claims.sub.len() > 512
        || claims.sub.chars().any(char::is_control)
        || SecretToken::parse(&claims.nonce)?.hash() != transaction.nonce_hash
    {
        return Err(AuthError::Invalid);
    }
    if let Some(hash) = claims.at_hash {
        let access = access_token
            .filter(|t| bounded_text(t, 8192))
            .ok_or(AuthError::Invalid)?;
        if hash != URL_SAFE_NO_PAD.encode(&Sha256::digest(access.as_bytes())[..16]) {
            return Err(AuthError::Invalid);
        }
    }
    Ok(Zeroizing::new(claims.sub))
}

pub(super) fn decode_signed<T: serde::de::DeserializeOwned>(
    jwks: &[u8],
    token: &str,
    audience: &str,
    issuers: &[&str],
    required: &[&str],
) -> Result<T, AuthError> {
    let kid = token_kid(token)?;
    let set = parse_jwks(jwks)?;
    let jwk = set.find(&kid).ok_or(AuthError::Invalid)?;
    if jwk
        .common
        .key_algorithm
        .is_some_and(|a| a != KeyAlgorithm::RS256)
        || jwk
            .common
            .public_key_use
            .as_ref()
            .is_some_and(|u| u != &PublicKeyUse::Signature)
        || jwk
            .common
            .key_operations
            .as_ref()
            .is_some_and(|ops| ops.as_slice() != [KeyOperations::Verify])
    {
        return Err(AuthError::Invalid);
    }
    let AlgorithmParameters::RSA(rsa) = &jwk.algorithm else {
        return Err(AuthError::Invalid);
    };
    let n = URL_SAFE_NO_PAD
        .decode(&rsa.n)
        .map_err(|_| AuthError::Invalid)?;
    let e = URL_SAFE_NO_PAD
        .decode(&rsa.e)
        .map_err(|_| AuthError::Invalid)?;
    if !(256..=1024).contains(&n.len())
        || (n.len() == 256 && n[0] < 128)
        || e.is_empty()
        || e.len() > 8
        || e[e.len() - 1] % 2 == 0
    {
        return Err(AuthError::Invalid);
    }
    let key = DecodingKey::from_jwk(jwk).map_err(|_| AuthError::Invalid)?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_required_spec_claims(required);
    validation.set_audience(&[audience]);
    validation.set_issuer(issuers);
    // Time is checked below with the same injected server clock as the transaction.
    validation.validate_exp = false;
    jsonwebtoken::decode::<T>(token, &key, &validation)
        .map(|data| data.claims)
        .map_err(|_| AuthError::Invalid)
}
