use liar_core::{
    daily::{DailyMetadata, DailyPuzzle},
    rules::RulesSnapshot,
};
use liar_protocol::{daily::DailyReplay, game::PublicAction};

#[derive(Clone, Debug)]
pub struct PublishedDaily {
    pub(super) metadata: DailyMetadata,
    pub(super) rules: RulesSnapshot,
}
impl PublishedDaily {
    pub fn new(date: &str, version: u16, rules: RulesSnapshot) -> Result<Self, ReplayError> {
        if rules.rules.max_commands_per_seat > RulesSnapshot::bundled().rules.max_commands_per_seat
        {
            return Err(ReplayError::InvalidDefinition);
        }
        let metadata = DailyMetadata::derive(date, version, &rules)
            .map_err(|_| ReplayError::InvalidDefinition)?;
        Ok(Self { metadata, rules })
    }
    pub fn metadata(&self) -> &DailyMetadata {
        &self.metadata
    }
    pub fn rules(&self) -> &RulesSnapshot {
        &self.rules
    }
    pub(super) fn validate(&self, replay: &DailyReplay) -> Result<(), ReplayError> {
        if replay.metadata != self.metadata {
            return Err(ReplayError::MetadataMismatch);
        }
        if replay.inputs.len() > usize::from(self.rules.rules.max_commands_per_seat) {
            return Err(ReplayError::InvalidReplay);
        }
        let mut last_time = 0;
        for entry in &replay.inputs {
            if entry.time_ms < last_time {
                return Err(ReplayError::InvalidReplay);
            }
            last_time = entry.time_ms;
            let cell = match entry.input.action {
                PublicAction::Open { cell } | PublicAction::Flag { cell } => cell,
                _ => return Err(ReplayError::InvalidReplay),
            };
            if usize::from(cell) >= self.rules.rules.board_spec().area() {
                return Err(ReplayError::InvalidReplay);
            }
        }
        if replay.final_time_ms < last_time {
            return Err(ReplayError::InvalidReplay);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayEvidence {
    pub(super) metadata: DailyMetadata,
    pub(super) opened_safe: usize,
    pub(super) mistakes: u16,
    pub(super) replayed_elapsed_ms: u32,
}
impl ReplayEvidence {
    pub(super) fn from_puzzle(puzzle: &DailyPuzzle) -> Result<Self, ReplayError> {
        if !puzzle.complete() {
            return Err(ReplayError::Incomplete);
        }
        let view = puzzle.projection();
        Ok(Self {
            metadata: puzzle.metadata().clone(),
            opened_safe: view.own.cells.opened_safe(),
            mistakes: view.own.stats.mistakes,
            replayed_elapsed_ms: puzzle.elapsed_ms(),
        })
    }
    pub fn metadata(&self) -> &DailyMetadata {
        &self.metadata
    }
    pub fn opened_safe(&self) -> usize {
        self.opened_safe
    }
    pub fn mistakes(&self) -> u16 {
        self.mistakes
    }
    pub fn replayed_elapsed_ms(&self) -> u32 {
        self.replayed_elapsed_ms
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayError {
    InvalidDefinition,
    Malformed,
    UnsupportedVersion,
    MetadataMismatch,
    InvalidReplay,
    Incomplete,
    Unavailable,
    Busy,
    Timeout,
}
pub trait ReplayVerifier: Send + Sync {
    fn verify(
        &self,
        definition: &PublishedDaily,
        source: &str,
    ) -> Result<ReplayEvidence, ReplayError>;
}
