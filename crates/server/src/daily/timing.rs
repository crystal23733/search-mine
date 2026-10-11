use super::{PublishedDaily, ReplayEvidence};
use liar_core::daily::DailyMetadata;
use uuid::Uuid;
const HEARTBEAT_MS: u32 = 15000;
const HEARTBEAT_GAP_MS: u64 = 45000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TicketError {
    Invalid,
    Conflict,
    Capacity,
    Unauthorized,
    Unavailable,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TicketGrant {
    pub id: Uuid,
    pub attempt: Uuid,
    pub metadata: DailyMetadata,
    pub ttl_ms: u64,
    pub heartbeat_interval_ms: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeartbeatAck {
    pub sequence: u32,
    pub elapsed_ms: u64,
}
#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedDailyTime {
    account: Uuid,
    attempt: Uuid,
    metadata: DailyMetadata,
    digest: [u8; 32],
    elapsed_ms: u32,
}
impl std::fmt::Debug for VerifiedDailyTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifiedDailyTime")
            .field("elapsed_ms", &self.elapsed_ms)
            .finish_non_exhaustive()
    }
}
impl VerifiedDailyTime {
    pub fn elapsed_ms(&self) -> u32 {
        self.elapsed_ms
    }
    pub fn matches(&self, account: Uuid, attempt: Uuid, evidence: &ReplayEvidence) -> bool {
        self.account == account
            && self.attempt == attempt
            && &self.metadata == evidence.metadata()
            && self.digest == evidence.fingerprint()
    }
}
pub(super) struct TimingAttempt {
    account: Uuid,
    grant: TicketGrant,
    started: u64,
    expires: u64,
    countdown: u64,
    observed: u64,
    last_seen: u64,
    last_sequence: u32,
    last_ack: Option<HeartbeatAck>,
    lost: bool,
    finished: Option<VerifiedDailyTime>,
}
impl TimingAttempt {
    pub fn new(
        account: Uuid,
        attempt: Uuid,
        ticket: Uuid,
        definition: &PublishedDaily,
        now: u64,
    ) -> Result<Self, TicketError> {
        if account.is_nil() || attempt.is_nil() || ticket.is_nil() {
            return Err(TicketError::Invalid);
        }
        let rules = &definition.rules().rules;
        let ttl_ms = u64::from(rules.countdown_ms) + u64::from(rules.duration_ms);
        let expires = now.checked_add(ttl_ms).ok_or(TicketError::Unavailable)?;
        Ok(Self {
            account,
            grant: TicketGrant {
                id: ticket,
                attempt,
                metadata: definition.metadata().clone(),
                ttl_ms,
                heartbeat_interval_ms: HEARTBEAT_MS,
            },
            started: now,
            expires,
            countdown: u64::from(rules.countdown_ms),
            observed: now,
            last_seen: now,
            last_sequence: 0,
            last_ack: None,
            lost: false,
            finished: None,
        })
    }
    pub fn grant(&self) -> &TicketGrant {
        &self.grant
    }
    pub fn expires_at(&self) -> u64 {
        self.expires
    }
    fn observe(&mut self, now: u64) -> Result<(), TicketError> {
        if now < self.observed {
            self.lost = true;
            self.finished = None;
            return Err(TicketError::Unavailable);
        }
        self.observed = now;
        if now >= self.expires || now - self.last_seen >= HEARTBEAT_GAP_MS {
            self.lost = true;
        }
        Ok(())
    }
    pub fn heartbeat(
        &mut self,
        sequence: u32,
        now: u64,
    ) -> Result<Option<HeartbeatAck>, TicketError> {
        if sequence == 0 {
            return Err(TicketError::Invalid);
        }
        self.observe(now)?;
        if self.lost || self.finished.is_some() {
            return Ok(None);
        }
        if sequence == self.last_sequence {
            return Ok(self.last_ack.clone());
        }
        if self.last_sequence.checked_add(1) != Some(sequence) {
            return Err(TicketError::Conflict);
        }
        let ack = HeartbeatAck {
            sequence,
            elapsed_ms: now - self.started,
        };
        self.last_seen = now;
        self.last_sequence = sequence;
        self.last_ack = Some(ack.clone());
        Ok(Some(ack))
    }
    pub fn finish(
        &mut self,
        evidence: &ReplayEvidence,
        now: u64,
    ) -> Result<Option<VerifiedDailyTime>, TicketError> {
        self.observe(now)?;
        if evidence.metadata() != &self.grant.metadata {
            return Err(TicketError::Invalid);
        }
        if now >= self.expires {
            return Ok(None);
        }
        if let Some(saved) = &self.finished {
            return if saved.matches(self.account, self.grant.attempt, evidence) {
                Ok(Some(saved.clone()))
            } else {
                Err(TicketError::Conflict)
            };
        }
        let elapsed = (now - self.started).checked_sub(self.countdown);
        if self.lost
            || elapsed.is_none_or(|elapsed| elapsed < u64::from(evidence.replayed_elapsed_ms()))
        {
            self.lost = true;
            return Ok(None);
        }
        let elapsed_ms = u32::try_from(elapsed.ok_or(TicketError::Unavailable)?)
            .map_err(|_| TicketError::Unavailable)?;
        let saved = VerifiedDailyTime {
            account: self.account,
            attempt: self.grant.attempt,
            metadata: self.grant.metadata.clone(),
            digest: evidence.fingerprint(),
            elapsed_ms,
        };
        self.finished = Some(saved.clone());
        Ok(Some(saved))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/daily_timing.rs"]
mod tests;
