use super::{PublishedDaily, ReplayError, ReplayEvidence, ReplayVerifier};
use liar_core::{
    board::CellId,
    daily::DailyPuzzle,
    game::{Action, ActionStatus, Command, Rejection, Seat},
};
use liar_protocol::{
    daily::decode_replay,
    game::{PublicAction, PublicError},
};

pub struct NativeReplayVerifier;
impl ReplayVerifier for NativeReplayVerifier {
    fn verify(
        &self,
        definition: &PublishedDaily,
        source: &str,
    ) -> Result<ReplayEvidence, ReplayError> {
        let replay = decode_replay(source).map_err(|error| match error {
            PublicError::UnsupportedVersion => ReplayError::UnsupportedVersion,
            PublicError::CommandLimit => ReplayError::InvalidReplay,
            _ => ReplayError::Malformed,
        })?;
        definition.validate(&replay)?;
        let canonical = serde_json::to_string(&replay).map_err(|_| ReplayError::Malformed)?;
        let mut puzzle = DailyPuzzle::with_rules(
            &definition.metadata.date,
            definition.metadata.seed_version,
            definition.rules.clone(),
        )
        .map_err(|_| ReplayError::Unavailable)?;
        for entry in replay.inputs {
            puzzle
                .advance(u64::from(entry.time_ms))
                .map_err(|_| ReplayError::InvalidReplay)?;
            let action = match entry.input.action {
                PublicAction::Open { cell } => Action::Open(CellId(cell)),
                PublicAction::Flag { cell } => Action::ToggleFlag(CellId(cell)),
                _ => return Err(ReplayError::InvalidReplay),
            };
            let ack = puzzle.apply(Command {
                id: u128::from(entry.input.command_id),
                seq: u64::from(entry.input.client_seq),
                epoch: 1,
                seat: Seat::One,
                received_at: u64::from(entry.time_ms),
                action,
            });
            match ack.status {
                ActionStatus::Rejected(Rejection::InvalidBoard | Rejection::InvalidRules) => {
                    return Err(ReplayError::Unavailable);
                }
                ActionStatus::Rejected(
                    Rejection::InvalidTime
                    | Rejection::InvalidEpoch
                    | Rejection::InvalidSequence
                    | Rejection::CommandConflict
                    | Rejection::CommandLimit
                    | Rejection::Disconnected
                    | Rejection::AttackUnavailable
                    | Rejection::AnalysisStale,
                ) => return Err(ReplayError::InvalidReplay),
                _ => {}
            }
        }
        puzzle
            .advance(u64::from(replay.final_time_ms))
            .map_err(|_| ReplayError::InvalidReplay)?;
        ReplayEvidence::from_puzzle(&puzzle, canonical)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/daily_replay.rs"]
mod tests;
