use super::{PublishedDaily, ReplayEvidence, timing::*};
use crate::{
    auth::{InvalidationBarrier, SessionInvalidator},
    online::{AuthorityRegistry, ConnectionAuthority, MatchClock},
};
use liar_protocol::online::OnlineError;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

pub struct DailyTicketRegistry {
    capacity: usize,
    authorities: Arc<AuthorityRegistry>,
    clock: Arc<dyn MatchClock>,
    inner: Mutex<Inner>,
}
struct Inner {
    observed: Option<u64>,
    failed: bool,
    entries: HashMap<Uuid, Entry>,
}
struct Entry {
    revision: Uuid,
    lease: ConnectionAuthority,
    timing: TimingAttempt,
}
impl Inner {
    fn observe(&mut self, current: u64, retired: &mut Vec<Entry>) -> Result<(), TicketError> {
        if self.failed || self.observed.is_some_and(|last| current < last) {
            self.failed = true;
            retired.extend(self.entries.drain().map(|(_, entry)| entry));
            return Err(TicketError::Unavailable);
        }
        self.observed = Some(current);
        let expired = self
            .entries
            .iter()
            .filter(|(_, entry)| {
                current >= entry.timing.expires_at() || *entry.lease.revoked().borrow()
            })
            .map(|(account, _)| *account)
            .collect::<Vec<_>>();
        for account in expired {
            if let Some(entry) = self.entries.remove(&account) {
                retired.push(entry);
            }
        }
        Ok(())
    }
}
fn authority(error: OnlineError) -> TicketError {
    if error == OnlineError::Unauthorized {
        TicketError::Unauthorized
    } else {
        TicketError::Unavailable
    }
}
impl DailyTicketRegistry {
    /// Reclaim expired leases before capturing generation and binding a new request.
    pub fn reap(&self) -> Result<(), TicketError> {
        let mut retired = Vec::new();
        let result = (|| {
            let mut inner = self.inner.lock().map_err(|_| TicketError::Unavailable)?;
            inner.observe(self.clock.now_ms(), &mut retired)
        })();
        drop(retired);
        result
    }
    pub fn new(
        capacity: usize,
        authorities: Arc<AuthorityRegistry>,
        clock: Arc<dyn MatchClock>,
    ) -> Result<Self, TicketError> {
        if !(1..=20000).contains(&capacity) {
            return Err(TicketError::Capacity);
        }
        Ok(Self {
            capacity,
            authorities,
            clock,
            inner: Mutex::new(Inner {
                observed: None,
                failed: false,
                entries: HashMap::new(),
            }),
        })
    }
    fn commit<T>(
        &self,
        lease: &ConnectionAuthority,
        now: i64,
        operation: impl FnOnce(&mut Inner, u64, &mut Vec<Entry>) -> Result<T, TicketError>,
    ) -> Result<T, TicketError> {
        let mut retired = Vec::new();
        let result = self.authorities.with_authority(lease, now, || {
            let mut inner = self.inner.lock().map_err(|_| TicketError::Unavailable)?;
            let current = self.clock.now_ms();
            inner.observe(current, &mut retired)?;
            operation(&mut inner, current, &mut retired)
        });
        // The last lease drop reenters AuthorityRegistry; release both locks first.
        drop(retired);
        result.map_err(authority)?
    }
    pub fn start(
        &self,
        lease: &ConnectionAuthority,
        revision: Uuid,
        attempt: Uuid,
        definition: &PublishedDaily,
        now: i64,
    ) -> Result<TicketGrant, TicketError> {
        if revision.is_nil() || attempt.is_nil() {
            return Err(TicketError::Invalid);
        }
        self.commit(lease, now, |inner, current, retired| {
            if let Some(entry) = inner.entries.get(&lease.account())
                && entry.timing.grant().attempt == attempt
            {
                if entry.revision != revision
                    || entry.lease.token() != lease.token()
                    || &entry.timing.grant().metadata != definition.metadata()
                {
                    return Err(TicketError::Conflict);
                }
                return Ok(entry.timing.grant().clone());
            }
            if inner.entries.len() >= self.capacity && !inner.entries.contains_key(&lease.account())
            {
                return Err(TicketError::Capacity);
            }
            let timing = TimingAttempt::new(
                lease.account(),
                attempt,
                Uuid::new_v4(),
                definition,
                current,
            )?;
            let grant = timing.grant().clone();
            if let Some(previous) = inner.entries.insert(
                lease.account(),
                Entry {
                    revision,
                    lease: lease.clone(),
                    timing,
                },
            ) {
                retired.push(previous);
            }
            Ok(grant)
        })
    }
    pub fn heartbeat(
        &self,
        lease: &ConnectionAuthority,
        revision: Uuid,
        ticket: Uuid,
        sequence: u32,
        now: i64,
    ) -> Result<Option<HeartbeatAck>, TicketError> {
        if revision.is_nil() || ticket.is_nil() || sequence == 0 {
            return Err(TicketError::Invalid);
        }
        self.commit(lease, now, |inner, current, _| {
            let Some(entry) = Self::entry(inner, lease, revision, ticket)? else {
                return Ok(None);
            };
            entry.timing.heartbeat(sequence, current)
        })
    }
    pub fn finish(
        &self,
        lease: &ConnectionAuthority,
        revision: Uuid,
        ticket: Uuid,
        attempt: Uuid,
        evidence: &ReplayEvidence,
        now: i64,
    ) -> Result<Option<VerifiedDailyTime>, TicketError> {
        if revision.is_nil() || ticket.is_nil() || attempt.is_nil() {
            return Err(TicketError::Invalid);
        }
        self.commit(lease, now, |inner, current, _| {
            let Some(entry) = Self::entry(inner, lease, revision, ticket)? else {
                return Ok(None);
            };
            if entry.timing.grant().attempt != attempt {
                return Err(TicketError::Conflict);
            }
            entry.timing.finish(evidence, current)
        })
    }
    fn entry<'a>(
        inner: &'a mut Inner,
        lease: &ConnectionAuthority,
        revision: Uuid,
        ticket: Uuid,
    ) -> Result<Option<&'a mut Entry>, TicketError> {
        let Some(entry) = inner
            .entries
            .get_mut(&lease.account())
            .filter(|entry| entry.timing.grant().id == ticket)
        else {
            return Ok(None);
        };
        if entry.revision != revision || entry.lease.token() != lease.token() {
            return Err(TicketError::Unauthorized);
        }
        Ok(Some(entry))
    }
    fn remove_revoked(&self) {
        let mut retired = Vec::new();
        if let Ok(mut inner) = self.inner.lock() {
            let revoked = inner
                .entries
                .iter()
                .filter(|(_, entry)| *entry.lease.revoked().borrow())
                .map(|(account, _)| *account)
                .collect::<Vec<_>>();
            for account in revoked {
                if let Some(entry) = inner.entries.remove(&account) {
                    retired.push(entry);
                }
            }
        }
        drop(retired);
    }
}
impl SessionInvalidator for DailyTicketRegistry {
    fn session(&self, hash: [u8; 32]) -> InvalidationBarrier {
        let barrier = self.authorities.session(hash);
        self.remove_revoked();
        barrier
    }
    fn account(&self, account: Uuid) -> InvalidationBarrier {
        let barrier = self.authorities.account(account);
        self.remove_revoked();
        barrier
    }
}

#[cfg(test)]
#[path = "../../tests/unit/daily_tickets.rs"]
mod tests;
