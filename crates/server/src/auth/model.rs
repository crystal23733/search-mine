use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use std::fmt;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthError {
    Invalid,
    Unavailable,
    Conflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Google,
    Apple,
    Kakao,
    Naver,
}
impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Apple => "apple",
            Self::Kakao => "kakao",
            Self::Naver => "naver",
        }
    }
    pub fn parse(value: &str) -> Result<Self, AuthError> {
        match value {
            "google" => Ok(Self::Google),
            "apple" => Ok(Self::Apple),
            "kakao" => Ok(Self::Kakao),
            "naver" => Ok(Self::Naver),
            _ => Err(AuthError::Invalid),
        }
    }
    pub fn issuer(self) -> &'static str {
        match self {
            Self::Google => "https://accounts.google.com",
            Self::Apple => "https://appleid.apple.com",
            Self::Kakao => "https://kauth.kakao.com",
            Self::Naver => "https://nid.naver.com",
        }
    }
    pub fn uses_pkce(self) -> bool {
        matches!(self, Self::Google | Self::Kakao)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nickname(String);
impl Nickname {
    pub fn parse(value: &str) -> Result<Self, AuthError> {
        if value.len() > 256 {
            return Err(AuthError::Invalid);
        }
        let value: String = value.nfc().collect();
        let graphemes: Vec<_> = value.graphemes(true).collect();
        if !(2..=16).contains(&graphemes.len())
            || value.len() > 128
            || value.starts_with(' ')
            || value.ends_with(' ')
        {
            return Err(AuthError::Invalid);
        }
        for g in graphemes {
            if g == " " {
                continue;
            }
            if !g.chars().next().is_some_and(char::is_alphanumeric) || g.chars().any(|c| {
                !(c.is_alphanumeric()||is_combining_mark(c)) || matches!(c,'\u{034f}'|'\u{180b}'..='\u{180f}'|'\u{fe00}'..='\u{fe0f}'|'\u{e0100}'..='\u{e01ef}')
            }) {return Err(AuthError::Invalid)}
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SecretToken([u8; 32]);
impl fmt::Debug for SecretToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretToken([REDACTED])")
    }
}
impl SecretToken {
    pub fn generate() -> Result<Self, AuthError> {
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|_| AuthError::Unavailable)?;
        Ok(Self(bytes))
    }
    pub fn parse(value: &str) -> Result<Self, AuthError> {
        if value.len() != 43 {
            return Err(AuthError::Invalid);
        }
        let bytes = Zeroizing::new(
            URL_SAFE_NO_PAD
                .decode(value)
                .map_err(|_| AuthError::Invalid)?,
        );
        let token = Self(
            bytes
                .as_slice()
                .try_into()
                .map_err(|_| AuthError::Invalid)?,
        );
        if token.expose().as_str() != value {
            return Err(AuthError::Invalid);
        }
        Ok(token)
    }
    pub fn expose(&self) -> Zeroizing<String> {
        Zeroizing::new(URL_SAFE_NO_PAD.encode(self.0))
    }
    pub fn hash(&self) -> [u8; 32] {
        Sha256::digest(self.0).into()
    }
    pub fn pkce_challenge(&self) -> String {
        URL_SAFE_NO_PAD.encode(Sha256::digest(self.expose().as_bytes()))
    }
}

pub const TRANSACTION_SECONDS: i64 = 300;
pub const SESSION_SECONDS: i64 = 30 * 24 * 60 * 60;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthIntent {
    Login,
    Link(Uuid),
    Reauth(Uuid),
}
impl AuthIntent {
    pub fn kind(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Link(_) => "link",
            Self::Reauth(_) => "reauth",
        }
    }
    pub fn account(self) -> Option<Uuid> {
        match self {
            Self::Login => None,
            Self::Link(id) | Self::Reauth(id) => Some(id),
        }
    }
    pub fn restore(kind: &str, account: Option<Uuid>) -> Result<Self, AuthError> {
        match (kind, account) {
            ("login", None) => Ok(Self::Login),
            ("link", Some(id)) => Ok(Self::Link(id)),
            ("reauth", Some(id)) => Ok(Self::Reauth(id)),
            _ => Err(AuthError::Invalid),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReturnPath {
    Home,
    Daily,
    Friends,
    Settings,
}
impl ReturnPath {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Daily => "daily",
            Self::Friends => "friends",
            Self::Settings => "settings",
        }
    }
    pub fn parse(value: &str) -> Result<Self, AuthError> {
        match value {
            "home" => Ok(Self::Home),
            "daily" => Ok(Self::Daily),
            "friends" => Ok(Self::Friends),
            "settings" => Ok(Self::Settings),
            _ => Err(AuthError::Invalid),
        }
    }
}
pub struct AuthTransaction {
    pub id: Uuid,
    pub state_hash: [u8; 32],
    pub browser_hash: [u8; 32],
    pub nonce_hash: [u8; 32],
    pub provider: Provider,
    pub intent: AuthIntent,
    pub return_path: ReturnPath,
    pub created_at: i64,
    pub expires_at: i64,
    pub encrypted_verifier: Option<Vec<u8>>,
}
impl AuthTransaction {
    pub fn matches(
        &self,
        state: &SecretToken,
        browser: &SecretToken,
        provider: Provider,
        now: i64,
    ) -> bool {
        self.state_hash == state.hash()
            && self.browser_hash == browser.hash()
            && self.provider == provider
            && now >= self.created_at
            && now < self.expires_at
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub id: Uuid,
    pub nickname: Option<Nickname>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub account: Account,
    pub created_at: i64,
    pub expires_at: i64,
    pub authenticated_at: i64,
}
impl Session {
    pub fn fresh(&self, now: i64, window: i64) -> bool {
        window > 0
            && now >= self.created_at
            && now < self.expires_at
            && now >= self.authenticated_at
            && now - self.authenticated_at < window
    }
}
