use super::*;
use axum::http::{HeaderMap, header};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

pub const BROWSER_COOKIE: &str = "__Host-liar_browser";
pub const SESSION_COOKIE: &str = "__Host-liar_session";
pub struct BrowserSecurity {
    origin: String,
    csrf_key: Zeroizing<[u8; 32]>,
}
impl BrowserSecurity {
    pub fn new(origin: &str, key: [u8; 32]) -> Result<Self, AuthError> {
        let url = url::Url::parse(origin).map_err(|_| AuthError::Invalid)?;
        if url.scheme() != "https"
            || !matches!(url.host(), Some(url::Host::Domain(_)))
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || key == [0; 32]
        {
            return Err(AuthError::Invalid);
        }
        Ok(Self {
            origin: url.origin().ascii_serialization(),
            csrf_key: Zeroizing::new(key),
        })
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
    fn mac(
        &self,
        browser: &SecretToken,
        session: Option<&SecretToken>,
        issued: i64,
    ) -> Result<Hmac<Sha256>, AuthError> {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.csrf_key.as_slice())
            .map_err(|_| AuthError::Unavailable)?;
        mac.update(b"liar.csrf.v1");
        mac.update(&issued.to_be_bytes());
        mac.update(&browser.hash());
        mac.update(&[u8::from(session.is_some())]);
        if let Some(session) = session {
            mac.update(&session.hash());
        }
        Ok(mac)
    }
    pub fn csrf(
        &self,
        browser: &SecretToken,
        session: Option<&SecretToken>,
        now: i64,
    ) -> Result<String, AuthError> {
        if now < 0 {
            return Err(AuthError::Invalid);
        }
        let mut value = now.to_be_bytes().to_vec();
        value.extend_from_slice(&self.mac(browser, session, now)?.finalize().into_bytes());
        Ok(URL_SAFE_NO_PAD.encode(value))
    }
    pub fn require(
        &self,
        headers: &HeaderMap,
        cookies: &BrowserCookies,
        now: i64,
    ) -> Result<(), AuthError> {
        if headers.get_all(header::ORIGIN).iter().count() != 1
            || headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(&self.origin)
        {
            return Err(AuthError::Invalid);
        }
        let browser = cookies.browser.as_ref().ok_or(AuthError::Invalid)?;
        if headers.get_all("x-liar-csrf").iter().count() != 1 {
            return Err(AuthError::Invalid);
        }
        let csrf = headers
            .get("x-liar-csrf")
            .and_then(|v| v.to_str().ok())
            .filter(|s| s.len() == 54)
            .ok_or(AuthError::Invalid)?;
        let bytes = URL_SAFE_NO_PAD
            .decode(csrf)
            .map_err(|_| AuthError::Invalid)?;
        if bytes.len() != 40 {
            return Err(AuthError::Invalid);
        }
        let issued = i64::from_be_bytes(bytes[..8].try_into().map_err(|_| AuthError::Invalid)?);
        if issued < 0 || now < issued || now - issued >= 300 {
            return Err(AuthError::Invalid);
        }
        self.mac(browser, cookies.session.as_ref(), issued)?
            .verify_slice(&bytes[8..])
            .map_err(|_| AuthError::Invalid)
    }
}
pub struct BrowserCookies {
    pub browser: Option<SecretToken>,
    pub session: Option<SecretToken>,
}
impl BrowserCookies {
    pub fn parse(headers: &HeaderMap) -> Result<Self, AuthError> {
        let mut browser = None;
        let mut session = None;
        let mut seen_browser = false;
        let mut seen_session = false;
        let mut size = 0usize;
        for header in headers.get_all(header::COOKIE) {
            let header = header.to_str().map_err(|_| AuthError::Invalid)?;
            size = size.saturating_add(header.len());
            if size > 8192 {
                return Err(AuthError::Invalid);
            }
            for pair in header.split(';') {
                let Some((name, value)) = pair.trim().split_once('=') else {
                    continue;
                };
                let target = match name {
                    BROWSER_COOKIE => {
                        if seen_browser {
                            return Err(AuthError::Invalid);
                        }
                        seen_browser = true;
                        &mut browser
                    }
                    SESSION_COOKIE => {
                        if seen_session {
                            return Err(AuthError::Invalid);
                        }
                        seen_session = true;
                        &mut session
                    }
                    _ => continue,
                };
                *target = Some(SecretToken::parse(value)?);
            }
        }
        Ok(Self { browser, session })
    }
}
pub(crate) fn cookie(name: &str, token: Option<&SecretToken>, age: i64) -> String {
    format!(
        "{name}={}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={age}",
        token.map(|t| t.expose().to_string()).unwrap_or_default()
    )
}
