use super::{AuthError, Provider};
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::BTreeMap;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};
struct Keyring {
    current: u32,
    keys: BTreeMap<u32, Zeroizing<[u8; 32]>>,
}
impl Keyring {
    fn new(current: u32, keys: Vec<(u32, [u8; 32])>) -> Result<Self, AuthError> {
        let mut map = BTreeMap::new();
        let mut invalid = keys.is_empty() || keys.len() > 4;
        for (version, mut key) in keys {
            invalid |= version == 0
                || version > i32::MAX as u32
                || key == [0; 32]
                || map.contains_key(&version);
            map.insert(version, Zeroizing::new(key));
            key.zeroize();
        }
        if invalid || !map.contains_key(&current) {
            return Err(AuthError::Invalid);
        }
        Ok(Self { current, keys: map })
    }
}
pub struct DigestKeys(Keyring);
impl DigestKeys {
    pub fn new(current: u32, keys: Vec<(u32, [u8; 32])>) -> Result<Self, AuthError> {
        Keyring::new(current, keys).map(Self)
    }
    pub fn digest(
        &self,
        provider: Provider,
        subject: &str,
    ) -> Result<Vec<(u32, [u8; 32])>, AuthError> {
        if subject.is_empty() || subject.len() > 512 || subject.chars().any(char::is_control) {
            return Err(AuthError::Invalid);
        }
        let mut result = Vec::new();
        for (version, key) in &self.0.keys {
            let mut mac =
                Hmac::<Sha256>::new_from_slice(key.as_slice()).map_err(|_| AuthError::Invalid)?;
            mac.update(b"liar.subject.v1");
            for value in [provider.as_str(), provider.issuer(), subject] {
                mac.update(&(value.len() as u64).to_be_bytes());
                mac.update(value.as_bytes());
            }
            result.push((*version, mac.finalize().into_bytes().into()));
        }
        result.sort_by_key(|(version, _)| *version != self.0.current);
        Ok(result)
    }
}
#[derive(Clone, Copy)]
pub enum CredentialPurpose {
    Pkce,
    AppleRevoke,
}
impl CredentialPurpose {
    fn aad(self, id: Uuid, provider: Provider) -> Result<Vec<u8>, AuthError> {
        if matches!(self, Self::AppleRevoke) && provider != Provider::Apple {
            return Err(AuthError::Invalid);
        }
        let purpose = match self {
            Self::Pkce => "pkce",
            Self::AppleRevoke => "apple-revoke",
        };
        Ok(format!("liar.credential.v1/{id}/{}/{purpose}", provider.as_str()).into_bytes())
    }
}
pub trait CredentialVault: Send + Sync {
    fn seal(
        &self,
        id: Uuid,
        provider: Provider,
        purpose: CredentialPurpose,
        value: &[u8],
    ) -> Result<Vec<u8>, AuthError>;
    fn open(
        &self,
        id: Uuid,
        provider: Provider,
        purpose: CredentialPurpose,
        value: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, AuthError>;
}
pub struct AeadVault(Keyring);
impl AeadVault {
    pub fn new(current: u32, keys: Vec<(u32, [u8; 32])>) -> Result<Self, AuthError> {
        Keyring::new(current, keys).map(Self)
    }
}
impl CredentialVault for AeadVault {
    fn seal(
        &self,
        id: Uuid,
        provider: Provider,
        purpose: CredentialPurpose,
        value: &[u8],
    ) -> Result<Vec<u8>, AuthError> {
        if value.is_empty() || value.len() > 4096 {
            return Err(AuthError::Invalid);
        }
        let aad = purpose.aad(id, provider)?;
        let key = &self.0.keys[&self.0.current];
        let cipher = Aes256Gcm::new_from_slice(key.as_slice()).map_err(|_| AuthError::Invalid)?;
        let mut nonce = [0; 12];
        getrandom::fill(&mut nonce).map_err(|_| AuthError::Unavailable)?;
        let encrypted = cipher
            .encrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: value,
                    aad: &aad,
                },
            )
            .map_err(|_| AuthError::Unavailable)?;
        let mut result = vec![1];
        result.extend(self.0.current.to_be_bytes());
        result.extend(nonce);
        result.extend(encrypted);
        Ok(result)
    }
    fn open(
        &self,
        id: Uuid,
        provider: Provider,
        purpose: CredentialPurpose,
        value: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, AuthError> {
        if !(34..=4129).contains(&value.len()) || value[0] != 1 {
            return Err(AuthError::Invalid);
        }
        let version = u32::from_be_bytes(value[1..5].try_into().map_err(|_| AuthError::Invalid)?);
        let key = self.0.keys.get(&version).ok_or(AuthError::Invalid)?;
        let cipher = Aes256Gcm::new_from_slice(key.as_slice()).map_err(|_| AuthError::Invalid)?;
        let nonce: [u8; 12] = value[5..17].try_into().map_err(|_| AuthError::Invalid)?;
        let aad = purpose.aad(id, provider)?;
        cipher
            .decrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: &value[17..],
                    aad: &aad,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| AuthError::Invalid)
    }
}
