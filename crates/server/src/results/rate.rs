use liar_protocol::results::ResultError;
use std::collections::HashMap;
use uuid::Uuid;
struct Entry {
    start: i64,
    count: u8,
    seen: i64,
}
pub(super) struct ResultRate {
    sessions: HashMap<[u8; 32], Entry>,
    accounts: HashMap<Uuid, Entry>,
    capacity: usize,
    last: i64,
}
impl ResultRate {
    pub fn new(capacity: usize) -> Self {
        Self {
            sessions: HashMap::new(),
            accounts: HashMap::new(),
            capacity,
            last: 0,
        }
    }
    fn advance(&mut self, now: i64) -> Result<(), ResultError> {
        if now < 0 || now < self.last {
            return Err(ResultError::Unavailable);
        }
        self.last = now;
        self.sessions.retain(|_, entry| now - entry.seen < 60);
        self.accounts.retain(|_, entry| now - entry.seen < 60);
        Ok(())
    }
    fn permit<K: Eq + std::hash::Hash>(
        map: &mut HashMap<K, Entry>,
        key: K,
        capacity: usize,
        now: i64,
    ) -> Result<(), ResultError> {
        if map.len() >= capacity && !map.contains_key(&key) {
            return Err(ResultError::Unavailable);
        }
        let entry = map.entry(key).or_insert(Entry {
            start: now,
            count: 0,
            seen: now,
        });
        entry.seen = now;
        if now > entry.start {
            entry.start = now;
            entry.count = 0;
        }
        if entry.count >= 20 {
            return Err(ResultError::RateLimited);
        }
        entry.count += 1;
        Ok(())
    }
    pub fn session(&mut self, hash: [u8; 32], now: i64) -> Result<(), ResultError> {
        self.advance(now)?;
        Self::permit(&mut self.sessions, hash, self.capacity, now)
    }
    pub fn account(&mut self, account: Uuid, now: i64) -> Result<(), ResultError> {
        self.advance(now)?;
        Self::permit(&mut self.accounts, account, self.capacity, now)
    }
}
#[cfg(test)]
#[path = "../../tests/unit/result_rate.rs"]
mod tests;
