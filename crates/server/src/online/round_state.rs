use super::{FinishedRace, RacePlayerResult};
use crate::{auth::Nickname, rooms::RoundKey};
use liar_core::{
    board::CellId,
    game::{
        Action, ActionStatus, Command, Rejection,
        multiplayer::{MAX_RACE_PLAYERS, MIN_RACE_PLAYERS, RaceEngine},
    },
};
use liar_protocol::{
    game::{AckStatus, PublicAction, PublicError},
    online::OnlineError,
    results::ResultStats,
    rounds::{RoundInput, RoundPayload, RoundView, from_race_projection},
};
use uuid::Uuid;

pub struct RoundParticipant {
    pub account: Uuid,
    pub nickname: Nickname,
}

/// Network seats, public projections and result translation over the core race.
pub struct RoundState {
    key: RoundKey,
    engine: RaceEngine,
    seed: [u8; 8],
    accounts: Vec<Uuid>,
    nicknames: Vec<Option<String>>,
    epochs: Vec<u32>,
}

impl RoundState {
    pub fn new(
        key: RoundKey,
        engine: RaceEngine,
        seed: [u8; 8],
        members: Vec<RoundParticipant>,
    ) -> Result<Self, OnlineError> {
        let count = members.len();
        if key.room_id.is_nil()
            || key.round_id.is_nil()
            || key.generation == 0
            || !(MIN_RACE_PLAYERS..=MAX_RACE_PLAYERS).contains(&count)
            || engine.last_sequence(count - 1).is_none()
            || engine.last_sequence(count).is_some()
            || members.iter().enumerate().any(|(seat, member)| {
                member.account.is_nil()
                    || members[..seat]
                        .iter()
                        .any(|other| other.account == member.account)
            })
        {
            return Err(OnlineError::Malformed);
        }
        let (accounts, nicknames) = members
            .into_iter()
            .map(|member| (member.account, Some(member.nickname.as_str().to_owned())))
            .unzip();
        Ok(Self {
            key,
            engine,
            seed,
            accounts,
            nicknames,
            epochs: vec![1; count],
        })
    }

    pub fn attach(&mut self, seat: usize, at: u64) -> Result<u32, OnlineError> {
        let current = self.epochs.get_mut(seat).ok_or(OnlineError::NotMatched)?;
        let epoch = current.checked_add(1).ok_or(OnlineError::Unavailable)?;
        self.engine
            .resume(seat, epoch, at)
            .map_err(|_| OnlineError::InvalidEpoch)?;
        *current = epoch;
        Ok(epoch)
    }

    pub fn view(&self, seat: usize) -> Option<RoundView> {
        self.engine
            .projection(seat)
            .map(|projection| from_race_projection(&projection, &self.nicknames))
    }

    pub fn advance(&mut self, at: u64) -> Result<(), Rejection> {
        self.engine.advance(at)
    }

    pub(super) fn key(&self) -> RoundKey {
        self.key
    }
    pub(super) fn accounts(&self) -> &[Uuid] {
        &self.accounts
    }
    pub(super) fn starts_at(&self) -> u64 {
        self.engine.start_ms()
    }
    pub(super) fn last_seq(&self, seat: usize) -> u32 {
        self.engine
            .last_sequence(seat)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u32::MAX)
    }
    pub(super) fn revision(&self, seat: usize) -> u64 {
        self.engine.revision(seat).unwrap_or(0)
    }
    pub(super) fn cancel(&mut self, at: u64) {
        let _ = self.engine.cancel_before_start(at.min(self.starts_at()));
    }
    pub(super) fn disconnect(&mut self, seat: usize, at: u64) {
        let _ = self.engine.disconnect(seat, at);
    }
    pub(super) fn forfeit_many(&mut self, seats: &[usize], at: u64) {
        let _ = self.engine.forfeit_many(seats, at);
    }
    pub(super) fn anonymize(&mut self, seat: usize) {
        self.nicknames[seat] = None;
    }
    pub(super) fn proof(&self, seat: usize) -> Result<liar_core::game::AnalysisJob, Rejection> {
        self.engine.proof_work(seat)
    }
    pub(super) fn commit_proof(&mut self, result: liar_core::game::AnalysisResult) -> bool {
        self.engine.commit_proof(result).is_ok()
    }

    pub fn apply(&mut self, seat: usize, input: RoundInput, at: u64) -> RoundPayload {
        let Some(revision) = self.engine.revision(seat) else {
            return RoundPayload::Error {
                code: OnlineError::NotMatched,
            };
        };
        if input.round_id != self.key.round_id.to_string() {
            return RoundPayload::Error {
                code: OnlineError::WrongMatch,
            };
        }
        let Ok(id) = Uuid::parse_str(&input.command_id) else {
            return RoundPayload::Error {
                code: OnlineError::Malformed,
            };
        };
        if id.is_nil() {
            return RoundPayload::Error {
                code: OnlineError::Malformed,
            };
        }
        if input.known_revision > revision {
            return RoundPayload::Ack {
                command_id: input.command_id,
                revision,
                status: AckStatus::Rejected,
                error: Some(PublicError::Stale),
                duplicate: false,
            };
        }
        let ack = self.engine.apply(Command {
            id: id.as_u128(),
            seq: u64::from(input.client_seq),
            epoch: input.session_epoch,
            seat,
            received_at: at,
            action: core_action(input.action),
        });
        let (status, error) = match ack.status {
            ActionStatus::Applied => (AckStatus::Applied, None),
            ActionStatus::Rejected(error) => (AckStatus::Rejected, Some(error.into())),
        };
        RoundPayload::Ack {
            command_id: input.command_id,
            revision: ack.revision,
            status,
            error,
            duplicate: ack.duplicate,
        }
    }

    pub fn finished(&self) -> Option<FinishedRace> {
        let result = self.engine.result()?;
        let players = self
            .accounts
            .iter()
            .enumerate()
            .map(|(seat, &account)| {
                let standing = result.standings.iter().find(|entry| entry.seat == seat)?;
                let own = self.engine.projection(seat)?.own;
                Some(RacePlayerResult {
                    account,
                    rank: standing.rank,
                    forfeited: standing.forfeited,
                    stats: ResultStats {
                        opened_safe: standing.opened_safe,
                        mistakes: own.stats.mistakes,
                        accusation_attempts: own.stats.accusation_attempts,
                        correct_accusations: own.stats.correct_accusations,
                    },
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(FinishedRace {
            id: self.key.round_id,
            room_id: self.key.room_id,
            rules_hash: self.engine.projection(0)?.rules.hash,
            seed: self.seed,
            ended_ms: result.at.saturating_sub(self.engine.start_ms()),
            reason: result.reason.into(),
            players,
        })
    }
}

pub(super) fn core_action(action: PublicAction) -> Action {
    match action {
        PublicAction::Open { cell } => Action::Open(CellId(cell)),
        PublicAction::Flag { cell } => Action::ToggleFlag(CellId(cell)),
        PublicAction::Accuse { cell } => Action::Accuse(CellId(cell)),
        PublicAction::Attack => Action::Attack,
    }
}
