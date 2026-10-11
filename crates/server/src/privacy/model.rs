use super::*;
use crate::{auth::Session, results::StoredPersonalResult};
use liar_protocol::{
    daily::{DailyReplay, decode_replay},
    game::PROTOCOL_VERSION,
};
use sha2::{Digest, Sha256};

pub(crate) fn id(value: &str) -> Result<Uuid, ExportError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or(ExportError::Malformed)
}
pub(crate) fn proof(
    session: &Session,
    expected: ExportSession,
    now: i64,
) -> Result<(), ExportError> {
    if session.id.is_nil()
        || session.id != expected.revision
        || session.account.id != expected.account
        || expected.account.is_nil()
        || now < 0
        || session.created_at > now
        || session.authenticated_at < session.created_at
        || session.authenticated_at > now
        || session.expires_at <= now
    {
        return Err(ExportError::Unauthorized);
    }
    if now.saturating_sub(session.authenticated_at) >= 300 {
        return Err(ExportError::Reauth);
    }
    Ok(())
}
pub(crate) fn same(
    first: &Session,
    last: &Session,
    expected: ExportSession,
    now: i64,
) -> Result<(), ExportError> {
    proof(last, expected, now)?;
    if first.created_at != last.created_at
        || first.authenticated_at != last.authenticated_at
        || first.expires_at != last.expires_at
    {
        return Err(ExportError::Unauthorized);
    }
    Ok(())
}
pub(crate) fn record_id(
    record: &ExportRecord,
    stream: ExportStream,
    now: i64,
) -> Result<Uuid, ExportError> {
    match (stream, record) {
        (
            ExportStream::Online,
            ExportRecord::Online {
                result,
                recorded_at,
                retention_started_at,
            },
        ) => {
            let match_id = id(&result.match_id).map_err(|_| ExportError::Unavailable)?;
            if *recorded_at < 0
                || *recorded_at > now
                || *retention_started_at < 0
                || *retention_started_at > *recorded_at
                || now.saturating_sub(*retention_started_at) >= 7776000
            {
                return Err(ExportError::Unavailable);
            }
            StoredPersonalResult {
                account: match_id,
                match_id,
                rules_hash: result.rules_hash.clone(),
                end_elapsed_ms: result.end_elapsed_ms,
                reason: result.result.reason,
                outcome: result.result.outcome,
                own: result.own.clone(),
            }
            .project(match_id, match_id)
            .map_err(|_| ExportError::Unavailable)?;
            if result.result.completed != result.result.reason.completed() {
                return Err(ExportError::Unavailable);
            }
            Ok(match_id)
        }
        (
            ExportStream::Daily,
            ExportRecord::Daily {
                completion_id,
                attempt_id,
                metadata,
                opened_safe,
                mistakes,
                completed_at,
                replay_available,
                ..
            },
        ) => {
            let completion = id(completion_id).map_err(|_| ExportError::Unavailable)?;
            id(attempt_id).map_err(|_| ExportError::Unavailable)?;
            let wire_rules = liar_core::rules::RulesSnapshot::bundled();
            if liar_core::daily::DailyMetadata::derive(
                &metadata.date,
                metadata.seed_version,
                &wire_rules,
            )
            .is_err()
                || metadata.mode != "solo-v1"
                || !metadata
                    .seed
                    .parse::<u64>()
                    .is_ok_and(|seed| seed.to_string() == metadata.seed)
                || metadata.solver_version == 0
                || metadata.rng_version == 0
                || metadata.seed_version != 1
                || metadata.rules_hash.len() != 64
                || !metadata
                    .rules_hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || metadata.date.len() != 10
                || *opened_safe == 0
                || *opened_safe > 65535
                || *mistakes > wire_rules.rules.max_commands_per_seat
                || *completed_at <= 0
                || *completed_at > now
                || now.saturating_sub(*completed_at) >= 7776000
                || (*replay_available && now.saturating_sub(*completed_at) >= 604800)
            {
                return Err(ExportError::Unavailable);
            }
            Ok(completion)
        }
        _ => Err(ExportError::Unavailable),
    }
}
pub(crate) fn page(
    account: Uuid,
    stream: ExportStream,
    after: Option<Uuid>,
    mut records: Vec<ExportRecord>,
    now: i64,
) -> Result<GameExportPage, ExportError> {
    if account.is_nil() || records.len() > 51 {
        return Err(ExportError::Unavailable);
    }
    let mut previous = after.unwrap_or(Uuid::nil());
    for record in &records {
        let current = record_id(record, stream, now)?;
        if current <= previous {
            return Err(ExportError::Unavailable);
        }
        previous = current;
    }
    let more = records.len() > 50;
    records.truncate(50);
    let next = if more {
        Some(record_id(records.last().ok_or(ExportError::Unavailable)?, stream, now)?.to_string())
    } else {
        None
    };
    Ok(GameExportPage {
        v: PROTOCOL_VERSION,
        account_id: account.to_string(),
        stream,
        records,
        next,
    })
}
pub(crate) fn raw(
    source: String,
    digest: Vec<u8>,
    metadata: liar_core::daily::DailyMetadata,
) -> Result<DailyReplay, ExportError> {
    let replay = decode_replay(&source).map_err(|_| ExportError::Unavailable)?;
    let canonical = serde_json::to_vec(&replay).map_err(|_| ExportError::Unavailable)?;
    if replay.metadata != metadata || Sha256::digest(canonical).as_slice() != digest {
        return Err(ExportError::Unavailable);
    }
    Ok(replay)
}

#[cfg(test)]
#[path = "../../tests/unit/privacy_model.rs"]
mod tests;
