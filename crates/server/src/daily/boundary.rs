use super::*;
use crate::auth::Session;
use liar_protocol::daily::*;
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failure {
    Invalid,
    Unauthorized,
    Conflict,
    Expired,
    Unavailable,
    RateLimited,
}
impl From<ReplayError> for Failure {
    fn from(error: ReplayError) -> Self {
        match error {
            ReplayError::Busy | ReplayError::Timeout | ReplayError::Unavailable => {
                Self::Unavailable
            }
            _ => Self::Invalid,
        }
    }
}
impl From<DailyStoreError> for Failure {
    fn from(error: DailyStoreError) -> Self {
        match error {
            DailyStoreError::Invalid => Self::Invalid,
            DailyStoreError::Unauthorized => Self::Unauthorized,
            DailyStoreError::Conflict => Self::Conflict,
            DailyStoreError::Expired => Self::Expired,
            DailyStoreError::Unavailable => Self::Unavailable,
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Operation {
    Definition,
    Submit,
    Own,
}
pub(super) enum Input {
    Definition(String, u16),
    Submit(Uuid, DailyReplay, String),
    Own(liar_core::daily::DailyMetadata),
}
pub(super) struct Expected {
    pub account: Uuid,
    pub revision: Uuid,
    pub input: Input,
}
fn id(value: &str) -> Result<Uuid, ReplayError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or(ReplayError::Malformed)
}
fn closed<T: serde::de::DeserializeOwned + Serialize>(source: &[u8]) -> Result<T, ReplayError> {
    let value: T = serde_json::from_slice(source).map_err(|_| ReplayError::Malformed)?;
    if serde_json::to_value(&value).map_err(|_| ReplayError::Unavailable)?
        != serde_json::from_slice::<serde_json::Value>(source)
            .map_err(|_| ReplayError::Malformed)?
    {
        return Err(ReplayError::Malformed);
    }
    Ok(value)
}
pub(super) fn decode(source: Vec<u8>, operation: Operation) -> Result<Expected, ReplayError> {
    let max = if matches!(operation, Operation::Submit) {
        MAX_DAILY_REPLAY_BYTES + 4096
    } else {
        4096
    };
    if source.len() > max {
        return Err(ReplayError::Malformed);
    }
    let (v, account, revision, input) = match operation {
        Operation::Definition => {
            let value: DailyDefinitionInput = closed(&source)?;
            PublishedDaily::new(
                &value.date,
                value.seed_version,
                liar_core::rules::RulesSnapshot::bundled(),
            )?;
            (
                value.v,
                value.account_id,
                value.session_revision,
                Input::Definition(value.date, value.seed_version),
            )
        }
        Operation::Submit => {
            let value: DailySubmitInput = closed(&source)?;
            let replay =
                serde_json::to_string(&value.replay).map_err(|_| ReplayError::Unavailable)?;
            decode_replay(&replay).map_err(|_| ReplayError::Malformed)?;
            (
                value.v,
                value.account_id,
                value.session_revision,
                Input::Submit(id(&value.attempt_id)?, value.replay, replay),
            )
        }
        Operation::Own => {
            let value: DailyOwnInput = closed(&source)?;
            PublishedDaily::new(
                &value.metadata.date,
                value.metadata.seed_version,
                liar_core::rules::RulesSnapshot::bundled(),
            )?;
            (
                value.v,
                value.account_id,
                value.session_revision,
                Input::Own(value.metadata),
            )
        }
    };
    if v != 1 {
        return Err(ReplayError::UnsupportedVersion);
    }
    Ok(Expected {
        account: id(&account)?,
        revision: id(&revision)?,
        input,
    })
}
pub(super) fn valid(session: &Session, now: i64) -> Result<(), Failure> {
    if session.id.is_nil()
        || session.account.id.is_nil()
        || now <= 0
        || now < session.created_at
        || session.authenticated_at < session.created_at
        || now < session.authenticated_at
        || now >= session.expires_at
        || session.account.nickname.is_none()
    {
        return Err(Failure::Unauthorized);
    }
    Ok(())
}
pub(super) fn proof(session: &Session, expected: &Expected, now: i64) -> Result<(), Failure> {
    valid(session, now)?;
    if session.account.id != expected.account || session.id != expected.revision {
        return Err(Failure::Unauthorized);
    }
    Ok(())
}
pub(super) fn same(
    first: &Session,
    last: &Session,
    expected: &Expected,
    now: i64,
) -> Result<(), Failure> {
    proof(last, expected, now)?;
    if first.created_at != last.created_at
        || first.authenticated_at != last.authenticated_at
        || first.expires_at != last.expires_at
    {
        return Err(Failure::Unauthorized);
    }
    Ok(())
}
pub(super) fn observe(
    clock: &dyn crate::auth::AuthClock,
    watermark: &mut i64,
) -> Result<i64, Failure> {
    let now = clock.now();
    if now < *watermark {
        return Err(Failure::Unavailable);
    }
    *watermark = now;
    Ok(now)
}
pub(super) fn completion(
    receipt: &DailyReceipt,
    definition: &PublishedDaily,
    now: i64,
) -> Result<DailyCompletion, Failure> {
    receipt.validate(definition, now).map_err(Failure::from)?;
    if receipt.expired(now) {
        return Err(Failure::Unavailable);
    }
    Ok(DailyCompletion {
        completion_id: receipt.id.to_string(),
        attempt_id: receipt.attempt.to_string(),
        metadata: receipt.metadata.clone(),
        opened_safe: receipt.opened_safe,
        mistakes: receipt.mistakes,
        replayed_elapsed_ms: receipt.replayed_elapsed_ms,
        verified_elapsed_ms: receipt.verified_elapsed_ms,
        completed_at: receipt.completed_at,
    })
}

#[cfg(test)]
#[path = "../../tests/unit/daily_boundary.rs"]
mod tests;
