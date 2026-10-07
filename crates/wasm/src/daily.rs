//! Native/browser daily adapter with an explicit unverified replay export.
use liar_core::daily::DailyPuzzle;
use liar_protocol::{
    daily::{DailyReplay, DailyStep, DailyTimedInput, DailyView, from_daily},
    game::{PROTOCOL_VERSION, PublicError},
};
pub struct DailySession {
    puzzle: DailyPuzzle,
    inputs: Vec<DailyTimedInput>,
    time_ms: u32,
}
impl DailySession {
    pub fn new(date: &str, version: u16) -> Result<Self, PublicError> {
        let puzzle = DailyPuzzle::new(date, version).map_err(|error| match error {
            liar_core::daily::DailyError::InvalidDate => PublicError::Malformed,
            liar_core::daily::DailyError::UnsupportedVersion => PublicError::UnsupportedVersion,
            _ => PublicError::Unavailable,
        })?;
        Ok(Self {
            puzzle,
            inputs: Vec::new(),
            time_ms: 0,
        })
    }
    pub fn view(&self) -> DailyView {
        from_daily(&self.puzzle)
    }
    pub fn step(&mut self, source: &str, time_ms: u32) -> Result<DailyStep, PublicError> {
        let input = liar_protocol::game::decode_local(source)?;
        if self.inputs.len()
            >= usize::from(self.puzzle.projection().rules.rules.max_commands_per_seat)
        {
            return Err(PublicError::CommandLimit);
        }
        self.puzzle
            .advance(u64::from(time_ms))
            .map_err(PublicError::from)?;
        let ack = self.puzzle.apply(liar_core::game::Command {
            id: u128::from(input.command_id),
            seq: u64::from(input.client_seq),
            epoch: 1,
            seat: liar_core::game::Seat::One,
            received_at: u64::from(time_ms),
            action: crate::session::action_from_public(input.action.clone()),
        });
        self.inputs.push(DailyTimedInput {
            input: input.clone(),
            time_ms,
        });
        self.time_ms = time_ms;
        Ok(DailyStep {
            ack: liar_protocol::game::from_ack(ack, input.command_id),
            view: self.view(),
        })
    }
    pub fn advance(&mut self, time_ms: u32) -> Result<DailyView, PublicError> {
        self.puzzle
            .advance(u64::from(time_ms))
            .map_err(PublicError::from)?;
        self.time_ms = time_ms;
        Ok(self.view())
    }
    pub fn export_replay(&self) -> DailyReplay {
        DailyReplay {
            v: PROTOCOL_VERSION,
            metadata: self.puzzle.metadata().clone(),
            inputs: self.inputs.clone(),
            final_time_ms: self.time_ms,
        }
    }
    pub fn from_replay(source: &str) -> Result<Self, PublicError> {
        let replay = liar_protocol::daily::decode_replay(source)?;
        let mut session = Self::new(&replay.metadata.date, replay.metadata.seed_version)?;
        if session.puzzle.metadata() != &replay.metadata {
            return Err(PublicError::Malformed);
        }
        for entry in replay.inputs {
            session.step(
                &serde_json::to_string(&entry.input).map_err(|_| PublicError::Malformed)?,
                entry.time_ms,
            )?;
        }
        session.advance(replay.final_time_ms)?;
        Ok(session)
    }
}
pub fn checked_seed_version(value: f64) -> Result<u16, PublicError> {
    if value == f64::from(liar_core::daily::DAILY_SEED_VERSION) {
        Ok(liar_core::daily::DAILY_SEED_VERSION)
    } else {
        Err(PublicError::UnsupportedVersion)
    }
}
