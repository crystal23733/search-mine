use liar_protocol::lobby::LobbyErrorCode;
use std::collections::HashMap;
use uuid::Uuid;
struct Window {
    start: i64,
    count: u8,
}
impl Window {
    fn permit(&mut self, now: i64, duration: i64, maximum: u8) -> Result<(), LobbyErrorCode> {
        if now - self.start >= duration {
            self.start = now;
            self.count = 0;
        }
        if self.count >= maximum {
            return Err(LobbyErrorCode::RateLimited);
        }
        self.count += 1;
        Ok(())
    }
}
struct Entry {
    burst: Window,
    joins: Window,
    seen: i64,
}
impl Entry {
    fn new(now: i64) -> Self {
        Self {
            burst: Window {
                start: now,
                count: 0,
            },
            joins: Window {
                start: now,
                count: 0,
            },
            seen: now,
        }
    }
}
pub(super) struct LobbyRate {
    sessions: HashMap<[u8; 32], Entry>,
    accounts: HashMap<Uuid, Entry>,
    capacity: usize,
    last: i64,
}
impl LobbyRate {
    pub fn new(capacity: usize) -> Self {
        Self {
            sessions: HashMap::new(),
            accounts: HashMap::new(),
            capacity,
            last: 0,
        }
    }
    fn advance(&mut self, now: i64) -> Result<(), LobbyErrorCode> {
        if now < 0 || now < self.last {
            return Err(LobbyErrorCode::Unavailable);
        }
        self.last = now;
        self.sessions.retain(|_, entry| now - entry.seen < 60);
        self.accounts.retain(|_, entry| now - entry.seen < 60);
        Ok(())
    }
    pub fn session(&mut self, hash: [u8; 32], now: i64) -> Result<(), LobbyErrorCode> {
        self.advance(now)?;
        if self.sessions.len() >= self.capacity && !self.sessions.contains_key(&hash) {
            return Err(LobbyErrorCode::Unavailable);
        }
        let entry = self.sessions.entry(hash).or_insert_with(|| Entry::new(now));
        entry.seen = now;
        entry.burst.permit(now, 1, 20)
    }
    pub fn account(&mut self, account: Uuid, join: bool, now: i64) -> Result<(), LobbyErrorCode> {
        self.advance(now)?;
        if self.accounts.len() >= self.capacity && !self.accounts.contains_key(&account) {
            return Err(LobbyErrorCode::Unavailable);
        }
        let entry = self
            .accounts
            .entry(account)
            .or_insert_with(|| Entry::new(now));
        entry.seen = now;
        entry.burst.permit(now, 1, 20)?;
        if join {
            entry.joins.permit(now, 60, 5)?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saturation_preserves_live_limits_and_exact_expiry_frees_both_maps() {
        let mut rate = LobbyRate::new(2);
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        for (hash, account) in [([1; 32], a), ([2; 32], b)] {
            rate.session(hash, 100).unwrap();
            for _ in 0..5 {
                rate.account(account, true, 100).unwrap();
            }
        }
        assert_eq!(rate.session([3; 32], 100), Err(LobbyErrorCode::Unavailable));
        assert_eq!(
            rate.account(c, false, 100),
            Err(LobbyErrorCode::Unavailable)
        );
        assert_eq!(rate.account(a, true, 101), Err(LobbyErrorCode::RateLimited));
        rate.session([3; 32], 160).unwrap();
        rate.account(c, false, 160).unwrap();
        rate.account(a, true, 160).unwrap();
        assert_eq!(rate.session([4; 32], 159), Err(LobbyErrorCode::Unavailable));
    }
}
