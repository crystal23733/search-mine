//! Native/browser training adapter; all lesson and game transitions stay in Rust.
use liar_core::tutorial::TutorialSession;
use liar_protocol::{
    game::{AckStatus, LocalAck, LocalInput, PublicError},
    tutorial::{TutorialStep, TutorialView, from_tutorial},
};
use std::collections::BTreeMap;
pub struct TrainingSession {
    session: TutorialSession,
    rejected: BTreeMap<u32, (LocalInput, LocalAck)>,
}
impl TrainingSession {
    pub fn new() -> Result<Self, PublicError> {
        Ok(Self {
            session: TutorialSession::new().map_err(PublicError::from)?,
            rejected: BTreeMap::new(),
        })
    }
    pub fn view(&self) -> TutorialView {
        from_tutorial(&self.session)
    }
    pub fn step(&mut self, input: &str, time_ms: u32) -> Result<TutorialStep, PublicError> {
        let input = liar_protocol::game::decode_local(input)?;
        self.session
            .advance(u64::from(time_ms))
            .map_err(PublicError::from)?;
        if let Some((original, ack)) = self.rejected.get(&input.command_id) {
            let ack = if original == &input {
                LocalAck {
                    duplicate: true,
                    ..ack.clone()
                }
            } else {
                LocalAck {
                    command_id: input.command_id,
                    revision: self.view().game.revision,
                    status: AckStatus::Rejected,
                    error: Some(PublicError::Conflict),
                    duplicate: false,
                }
            };
            return Ok(TutorialStep {
                ack,
                view: self.view(),
            });
        }
        if self.rejected.len()
            >= usize::from(self.session.projection().rules.rules.max_commands_per_seat)
        {
            return Err(PublicError::CommandLimit);
        }
        let ack = self.session.apply(liar_core::game::Command {
            id: u128::from(input.command_id),
            seq: u64::from(input.client_seq),
            epoch: 1,
            seat: liar_core::game::Seat::One,
            received_at: u64::from(time_ms),
            action: crate::session::action_from_public(input.action.clone()),
        });
        let ack = match ack {
            Ok(ack) => liar_protocol::game::from_ack(ack, input.command_id),
            Err(liar_core::game::Rejection::InvalidCell) => {
                let ack = LocalAck {
                    command_id: input.command_id,
                    revision: self.view().game.revision,
                    status: AckStatus::Rejected,
                    error: Some(PublicError::InvalidCell),
                    duplicate: false,
                };
                self.rejected.insert(input.command_id, (input, ack.clone()));
                ack
            }
            Err(error) => return Err(PublicError::from(error)),
        };
        Ok(TutorialStep {
            ack,
            view: self.view(),
        })
    }
    pub fn advance(&mut self, time_ms: u32) -> Result<TutorialView, PublicError> {
        self.session
            .advance(u64::from(time_ms))
            .map_err(PublicError::from)?;
        Ok(self.view())
    }
}
