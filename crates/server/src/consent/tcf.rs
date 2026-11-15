//! Bounded TCF v2 reader. Vendor ranges are inspected, never expanded into sets.
use super::ConsentError;
use liar_protocol::consent::*;
struct Bits<'a> {
    source: &'a [u8],
    position: usize,
}
impl<'a> Bits<'a> {
    fn new(source: &'a str) -> Result<Self, ConsentError> {
        if source.is_empty()
            || !source
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        {
            return Err(ConsentError::Invalid);
        }
        Ok(Self {
            source: source.as_bytes(),
            position: 0,
        })
    }
    fn remaining(&self) -> usize {
        self.source.len() * 6 - self.position
    }
    fn bit(&self, position: usize) -> u64 {
        let value = match self.source[position / 6] {
            b'A'..=b'Z' => self.source[position / 6] - b'A',
            b'a'..=b'z' => self.source[position / 6] - b'a' + 26,
            b'0'..=b'9' => self.source[position / 6] - b'0' + 52,
            b'-' => 62,
            _ => 63,
        };
        u64::from((value >> (5 - position % 6)) & 1)
    }
    fn read(&mut self, n: usize) -> Result<u64, ConsentError> {
        if n > 64 || n > self.remaining() {
            return Err(ConsentError::Invalid);
        }
        let result = (0..n).fold(0, |value, i| (value << 1) | self.bit(self.position + i));
        self.position += n;
        Ok(result)
    }
    fn skip(&mut self, n: usize) -> Result<(), ConsentError> {
        if n > self.remaining() {
            return Err(ConsentError::Invalid);
        }
        self.position += n;
        Ok(())
    }
    fn done(&mut self) -> Result<(), ConsentError> {
        let remaining = self.remaining();
        if remaining > 5 || self.read(remaining)? != 0 {
            return Err(ConsentError::Invalid);
        }
        Ok(())
    }
    fn ranges(&mut self, max: u64) -> Result<bool, ConsentError> {
        let count = self.read(12)? as usize;
        if count > self.remaining() / 17 {
            return Err(ConsentError::Invalid);
        }
        let mut previous = 0;
        let mut contains = false;
        for _ in 0..count {
            let range = self.read(1)? != 0;
            let start = self.read(16)?;
            let end = if range { self.read(16)? } else { start };
            if start == 0 || start <= previous || end < start || end > max {
                return Err(ConsentError::Invalid);
            }
            contains |= start <= u64::from(GOOGLE_VENDOR_ID) && u64::from(GOOGLE_VENDOR_ID) <= end;
            previous = end;
        }
        Ok(contains)
    }
    fn vendors(&mut self) -> Result<bool, ConsentError> {
        let max = self.read(16)?;
        if self.read(1)? != 0 {
            return self.ranges(max);
        }
        if max as usize > self.remaining() {
            return Err(ConsentError::Invalid);
        }
        let contains = max >= u64::from(GOOGLE_VENDOR_ID)
            && self.bit(self.position + usize::from(GOOGLE_VENDOR_ID) - 1) != 0;
        self.skip(max as usize)?;
        Ok(contains)
    }
}
fn all(flags: u64, purposes: &[u16]) -> bool {
    purposes.iter().all(|&id| flags & (1 << (24 - id)) != 0)
}
fn core(source: &str, now: i64) -> Result<bool, ConsentError> {
    let mut b = Bits::new(source)?;
    if b.read(6)? != 2 {
        return Err(ConsentError::Invalid);
    }
    let created = b.read(36)?;
    let updated = b.read(36)?;
    if created == 0
        || created > updated
        || updated / 10 > now as u64
        || b.read(12)? != u64::from(GOOGLE_CMP_ID)
    {
        return Err(ConsentError::Invalid);
    }
    b.skip(12 + 6)?;
    for _ in 0..2 {
        if b.read(6)? > 25 {
            return Err(ConsentError::Invalid);
        }
    }
    if b.read(12)? == 0 {
        return Err(ConsentError::Invalid);
    }
    if !TCF_POLICY_VERSIONS.contains(&(b.read(6)? as u8)) || b.read(1)? != 1 {
        return Err(ConsentError::Invalid);
    }
    b.skip(1 + 12)?;
    let purposes = b.read(24)?;
    b.skip(24 + 1)?;
    for _ in 0..2 {
        if b.read(6)? > 25 {
            return Err(ConsentError::Invalid);
        }
    }
    let mut google = b.vendors()? && all(purposes, &GOOGLE_AD_PURPOSES);
    let _ = b.vendors()?;
    let restrictions = b.read(12)? as usize;
    if restrictions > b.remaining() / 20 {
        return Err(ConsentError::Invalid);
    }
    for _ in 0..restrictions {
        let purpose = b.read(6)? as u16;
        let kind = b.read(2)?;
        if !(1..=24).contains(&purpose) || kind == 3 {
            return Err(ConsentError::Invalid);
        }
        if b.ranges(65535)? && GOOGLE_AD_PURPOSES.contains(&purpose) && kind != 1 {
            google = false;
        }
    }
    b.done()?;
    Ok(google)
}
fn verify_tcf(raw: &str, now: i64) -> Result<(bool, bool), ConsentError> {
    if !(0..=crate::events::MAX_EVENT_TIMESTAMP).contains(&now) || raw.len() > MAX_TCF_CHARS {
        return Err(ConsentError::Invalid);
    }
    let mut segments = raw.split('.');
    let google = core(segments.next().ok_or(ConsentError::Invalid)?, now)?;
    let mut seen = 0u8;
    let mut analytics = false;
    let mut advertising = false;
    for source in segments {
        let mut b = Bits::new(source)?;
        let kind = b.read(3)? as u8;
        if !(1..=3).contains(&kind) || seen & (1 << kind) != 0 {
            return Err(ConsentError::Invalid);
        }
        seen |= 1 << kind;
        match kind {
            1 | 2 => {
                let _ = b.vendors()?;
            }
            _ => {
                let consent = b.read(24)?;
                b.skip(24)?;
                let custom = b.read(6)? as usize;
                b.skip(custom * 2)?;
                analytics = all(consent, &GOOGLE_ANALYTICS_PURPOSES);
                advertising = google && all(consent, &GOOGLE_MEASUREMENT_PURPOSES);
            }
        }
        b.done()?;
    }
    Ok((analytics, advertising))
}
/// The current user submits a choice; this is not a signed Google attestation.
pub struct GoogleConsentVerifier {
    enabled: bool,
}
impl GoogleConsentVerifier {
    pub fn from_provider(provider: Option<&str>) -> Result<Self, ConsentError> {
        match provider {
            None | Some("") => Ok(Self { enabled: false }),
            Some("google") => Ok(Self { enabled: true }),
            _ => Err(ConsentError::Invalid),
        }
    }
}
impl super::ConsentVerifier for GoogleConsentVerifier {
    async fn verify(&self, _: uuid::Uuid, _: i64) -> Result<super::VerifiedConsent, ConsentError> {
        Err(ConsentError::Unavailable)
    }
    async fn verify_submitted(
        &self,
        subject: uuid::Uuid,
        now: i64,
        evidence: Option<&ConsentEvidence>,
    ) -> Result<super::VerifiedConsent, ConsentError> {
        if !self.enabled {
            return Err(ConsentError::Unavailable);
        }
        let evidence = evidence.ok_or(ConsentError::Unavailable)?;
        let (analytics, advertising) = verify_tcf(&evidence.tc_string, now)?;
        let expires_at = now
            .checked_add(3600)
            .filter(|&at| at <= crate::events::MAX_EVENT_TIMESTAMP)
            .ok_or(ConsentError::Invalid)?;
        Ok(super::VerifiedConsent {
            subject,
            analytics,
            advertising,
            expires_at,
        })
    }
}
#[cfg(test)]
#[path = "../../tests/unit/google_tcf.rs"]
mod tests;
