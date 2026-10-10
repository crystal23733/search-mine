use crate::{auth::Session, online::PortFuture};
use liar_protocol::{
    game::{GameResult, Outcome, PublicEndReason},
    results::*,
};
use uuid::Uuid;

// Internal subject binding is deliberately not serializable or printable.
#[derive(Clone)]
pub struct StoredPersonalResult {
    pub account: Uuid,
    pub match_id: Uuid,
    pub rules_hash: String,
    pub end_elapsed_ms: Option<u64>,
    pub reason: PublicEndReason,
    pub outcome: Outcome,
    pub own: Option<ResultStats>,
}
pub trait ResultReader: Send + Sync {
    fn read(
        &self,
        account: Uuid,
        match_id: Uuid,
    ) -> PortFuture<'_, Result<Option<StoredPersonalResult>, ResultError>>;
}
impl StoredPersonalResult {
    pub fn project(self, account: Uuid, match_id: Uuid) -> Result<PersonalResult, ResultError> {
        let compatible = match self.reason {
            PublicEndReason::ServerFailure => self.outcome == Outcome::Abort,
            PublicEndReason::Cancelled => self.outcome == Outcome::Cancelled,
            _ => matches!(self.outcome, Outcome::Win | Outcome::Loss | Outcome::Draw),
        };
        let details_valid = match (&self.end_elapsed_ms, &self.own) {
            (Some(elapsed), Some(own)) => {
                *elapsed <= 9007199254740991
                    && own.opened_safe <= 256
                    && own.mistakes <= 4096
                    && own.accusation_attempts <= 4096
                    && own.correct_accusations <= own.accusation_attempts
            }
            (None, None) => {
                self.reason == PublicEndReason::ServerFailure && self.outcome == Outcome::Abort
            }
            _ => false,
        };
        if self.account.is_nil()
            || self.match_id.is_nil()
            || self.account != account
            || self.match_id != match_id
            || self.rules_hash.len() != 64
            || !self
                .rules_hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !details_valid
            || !compatible
        {
            return Err(ResultError::Unavailable);
        }
        Ok(PersonalResult {
            match_id: self.match_id.to_string(),
            rules_hash: self.rules_hash,
            end_elapsed_ms: self.end_elapsed_ms,
            result: GameResult {
                reason: self.reason,
                outcome: self.outcome,
                completed: self.reason.completed(),
            },
            own: self.own,
        })
    }
}
pub(super) fn valid_session(session: &Session, now: i64) -> bool {
    !session.id.is_nil()
        && !session.account.id.is_nil()
        && session.account.nickname.is_some()
        && now >= 0
        && now >= session.created_at
        && now >= session.authenticated_at
        && session.authenticated_at >= session.created_at
        && now < session.expires_at
}
pub(super) fn same_session(first: &Session, last: &Session, now: i64) -> bool {
    valid_session(last, now)
        && first.id == last.id
        && first.account.id == last.account.id
        && first.created_at == last.created_at
        && first.expires_at == last.expires_at
        && first.authenticated_at == last.authenticated_at
}

#[cfg(test)]
#[path = "../../tests/unit/result_model.rs"]
mod tests;
