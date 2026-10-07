use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sqlx::PgPool;
use std::sync::Arc;
use zeroize::Zeroizing;

pub struct RuntimeAuthConfig {
    pub security: BrowserSecurity,
    pub vault: AeadVault,
    pub digests: DigestKeys,
    pub providers: Vec<ProviderConfig>,
}
fn secret<F: Fn(&str) -> Option<String>>(
    read: &F,
    key: &str,
) -> Result<Zeroizing<String>, AuthError> {
    read(key)
        .filter(|s| !s.is_empty() && s.len() <= 16 * 1024)
        .map(Zeroizing::new)
        .ok_or(AuthError::Invalid)
}
fn key(value: &str) -> Result<[u8; 32], AuthError> {
    let bytes = Zeroizing::new(
        URL_SAFE_NO_PAD
            .decode(value)
            .map_err(|_| AuthError::Invalid)?,
    );
    bytes.as_slice().try_into().map_err(|_| AuthError::Invalid)
}
#[derive(serde::Deserialize, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
struct Keys {
    current: u32,
    keys: Vec<Key>,
}
#[derive(serde::Deserialize, zeroize::Zeroize)]
#[serde(deny_unknown_fields)]
struct Key {
    version: u32,
    key: String,
}
type KeyVersions = Vec<(u32, [u8; 32])>;
fn keys(value: &str) -> Result<(u32, KeyVersions), AuthError> {
    let keys: Keys = serde_json::from_str(value).map_err(|_| AuthError::Invalid)?;
    let values = keys
        .keys
        .iter()
        .map(|k| Ok((k.version, key(&k.key)?)))
        .collect::<Result<Vec<_>, AuthError>>()?;
    Ok((keys.current, values))
}
pub fn load_auth_config<
    F: Fn(&str) -> Option<String>,
    K: Fn(&str) -> Result<Zeroizing<Vec<u8>>, AuthError>,
>(
    read: F,
    read_key: K,
) -> Result<Option<RuntimeAuthConfig>, AuthError> {
    let names = [
        "LIAR_PUBLIC_ORIGIN",
        "LIAR_AUTH_CSRF_KEY",
        "LIAR_AUTH_DIGEST_KEYS",
        "LIAR_AUTH_VAULT_KEYS",
        "LIAR_GOOGLE_CLIENT_ID",
        "LIAR_GOOGLE_CLIENT_SECRET",
        "LIAR_KAKAO_CLIENT_ID",
        "LIAR_KAKAO_CLIENT_SECRET",
        "LIAR_NAVER_CLIENT_ID",
        "LIAR_NAVER_CLIENT_SECRET",
        "LIAR_APPLE_CLIENT_ID",
        "LIAR_APPLE_TEAM_ID",
        "LIAR_APPLE_KEY_ID",
        "LIAR_APPLE_PRIVATE_KEY_FILE",
    ];
    if !names.iter().any(|name| read(name).is_some()) {
        return Ok(None);
    }
    let origin = secret(&read, "LIAR_PUBLIC_ORIGIN")?;
    let security = BrowserSecurity::new(&origin, key(&secret(&read, "LIAR_AUTH_CSRF_KEY")?)?)?;
    let (current, values) = keys(&secret(&read, "LIAR_AUTH_DIGEST_KEYS")?)?;
    let digests = DigestKeys::new(current, values)?;
    let (current, values) = keys(&secret(&read, "LIAR_AUTH_VAULT_KEYS")?)?;
    let vault = AeadVault::new(current, values)?;
    let mut providers = Vec::new();
    for (provider, prefix) in [
        (Provider::Google, "GOOGLE"),
        (Provider::Apple, "APPLE"),
        (Provider::Kakao, "KAKAO"),
        (Provider::Naver, "NAVER"),
    ] {
        let id_name = format!("LIAR_{prefix}_CLIENT_ID");
        let secret_name = format!("LIAR_{prefix}_CLIENT_SECRET");
        let fields = if provider == Provider::Apple {
            vec![
                id_name.as_str(),
                "LIAR_APPLE_TEAM_ID",
                "LIAR_APPLE_KEY_ID",
                "LIAR_APPLE_PRIVATE_KEY_FILE",
            ]
        } else {
            vec![id_name.as_str(), secret_name.as_str()]
        };
        if !fields.iter().any(|name| read(name).is_some()) {
            continue;
        }
        let client_id = secret(&read, &id_name)?.to_string();
        let client_secret = if provider == Provider::Apple {
            Zeroizing::new(String::new())
        } else {
            secret(&read, &secret_name)?
        };
        let apple = if provider == Provider::Apple {
            Some(AppleSigning {
                team_id: secret(&read, "LIAR_APPLE_TEAM_ID")?.to_string(),
                key_id: secret(&read, "LIAR_APPLE_KEY_ID")?.to_string(),
                private_key: read_key(&secret(&read, "LIAR_APPLE_PRIVATE_KEY_FILE")?)?,
            })
        } else {
            None
        };
        let callback = url::Url::parse(&format!(
            "{}/api/v1/auth/{}/callback",
            security.origin(),
            provider.as_str()
        ))
        .map_err(|_| AuthError::Invalid)?;
        let config = ProviderConfig {
            provider,
            client_id,
            client_secret,
            callback,
            apple,
        };
        config.validate()?;
        providers.push(config);
    }
    Ok(Some(RuntimeAuthConfig {
        security,
        vault,
        digests,
        providers,
    }))
}
pub fn read_private_key(path: &str) -> Result<Zeroizing<Vec<u8>>, AuthError> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|_| AuthError::Invalid)?;
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(8193)
        .read_to_end(&mut bytes)
        .map_err(|_| AuthError::Invalid)?;
    if bytes.is_empty() || bytes.len() > 8192 {
        return Err(AuthError::Invalid);
    }
    Ok(bytes)
}
impl RuntimeAuthConfig {
    pub fn router(self, pool: PgPool) -> Result<axum::Router, AuthError> {
        let clock: Arc<dyn AuthClock> = Arc::new(SystemAuthClock);
        let registry =
            ProviderRegistry::new(HttpsOAuthTransport::new()?, clock.clone(), self.providers)?;
        Ok(auth_router(
            AuthService::new(PgAuthStore::new(pool), self.vault, self.digests),
            registry,
            self.security,
            clock,
        ))
    }
}
