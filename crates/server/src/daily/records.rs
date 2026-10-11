use super::{PublishedDaily, ReplayEvidence};
use liar_core::daily::DailyMetadata;
use std::{future::Future, pin::Pin};
use uuid::Uuid;

pub type DailyFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Copy)]
pub struct DailySession {
    pub account: Uuid,
    pub hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DailyReceipt {
    pub id: Uuid,
    pub attempt: Uuid,
    pub metadata: DailyMetadata,
    pub opened_safe: u32,
    pub mistakes: u16,
    pub replayed_elapsed_ms: u32,
    pub verified_elapsed_ms: Option<u32>,
    pub completed_at: i64,
}
impl DailyReceipt {
    pub(super) fn validate(
        &self,
        definition: &PublishedDaily,
        now: i64,
    ) -> Result<(), DailyStoreError> {
        let rules = &definition.rules().rules;
        if self.id.is_nil()
            || self.attempt.is_nil()
            || self.metadata != *definition.metadata()
            || self.opened_safe as usize != rules.board_spec().area() - usize::from(rules.mines)
            || self.mistakes > rules.max_commands_per_seat
            || self.verified_elapsed_ms.is_some_and(|elapsed| {
                elapsed < self.replayed_elapsed_ms || elapsed >= rules.duration_ms
            })
            || self.completed_at <= 0
            || self.completed_at > now
        {
            return Err(DailyStoreError::Unavailable);
        }
        Ok(())
    }
    pub(super) fn expired(&self, now: i64) -> bool {
        now.saturating_sub(self.completed_at) >= 90 * 86400
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DailySubmission {
    Official(DailyReceipt),
    Practice(DailyReceipt),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DailyStoreError {
    Invalid,
    Conflict,
    Expired,
    Unauthorized,
    Unavailable,
}
pub trait DailyRepository: Send + Sync {
    fn publish(
        &self,
        candidate: PublishedDaily,
    ) -> DailyFuture<'_, Result<PublishedDaily, DailyStoreError>>;
    fn definition<'a>(
        &'a self,
        date: &'a str,
        version: u16,
    ) -> DailyFuture<'a, Result<Option<PublishedDaily>, DailyStoreError>>;
    fn submit<'a>(
        &'a self,
        session: DailySession,
        attempt: Uuid,
        evidence: &'a ReplayEvidence,
    ) -> DailyFuture<'a, Result<DailySubmission, DailyStoreError>>;
    fn submit_timed<'a>(
        &'a self,
        _session: DailySession,
        _attempt: Uuid,
        _evidence: &'a ReplayEvidence,
        _time: &'a super::VerifiedDailyTime,
    ) -> DailyFuture<'a, Result<DailySubmission, DailyStoreError>> {
        Box::pin(async { Err(DailyStoreError::Unavailable) })
    }
    fn own<'a>(
        &'a self,
        session: DailySession,
        metadata: &'a DailyMetadata,
    ) -> DailyFuture<'a, Result<Option<DailyReceipt>, DailyStoreError>>;
}

#[cfg(test)]
#[path = "../../tests/unit/daily_records.rs"]
mod tests;
