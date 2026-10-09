//! Pure application state. Secret engine and result metadata never implement Serialize or Debug.
use liar_core::{
    board::CellId,
    game::{
        Action, ActionStatus, AnalysisJob, AnalysisResult, Command, Rejection, RuleEngine, Seat,
    },
};
use liar_protocol::{
    game::{
        AckStatus, CellState, GameView, Outcome, PublicAction, PublicEndReason, PublicError,
        from_projection,
    },
    online::{OnlineError, OnlineInput, OnlinePayload},
};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct PlayerResult {
    pub account: Option<Uuid>,
    pub outcome: Outcome,
    pub opened_safe: u16,
    pub mistakes: u16,
    pub accusations: u16,
    pub correct_accusations: u16,
}
#[derive(Clone, PartialEq, Eq)]
pub struct FinishedMatch {
    pub id: Uuid,
    pub rules_hash: String,
    pub seed: [u8; 8],
    pub ended_ms: u64,
    pub reason: PublicEndReason,
    pub players: [PlayerResult; 2],
}
#[derive(PartialEq, Eq)]
struct Visible {
    view: GameView,
    own_stun_until: u64,
    opponent_stun_until: u64,
}
impl Visible {
    fn new(mut view: GameView, at: u64) -> Self {
        let own_stun_until = if view.own.stun_ms == 0 {
            0
        } else {
            at.saturating_add(u64::from(view.own.stun_ms))
        };
        let opponent_stun_until = if view.opponent.stun_ms == 0 {
            0
        } else {
            at.saturating_add(u64::from(view.opponent.stun_ms))
        };
        view.revision = 0;
        view.countdown_ms = 0;
        view.remaining_ms = 0;
        view.own.stun_ms = 0;
        view.opponent.stun_ms = 0;
        Self {
            view,
            own_stun_until,
            opponent_stun_until,
        }
    }
}
pub struct MatchState {
    id: Uuid,
    engine: RuleEngine,
    players: [Option<Uuid>; 2],
    seed: [u8; 8],
    epochs: [u32; 2],
    revisions: [u32; 2],
    visible: [Visible; 2],
    changed: [bool; 2],
    now: u64,
    created_at: u64,
    acknowledgements: [HashMap<Uuid, u32>; 2],
}
pub(super) struct BotIntent {
    pub id: Uuid,
    pub seq: u32,
    pub action: Action,
}
impl MatchState {
    pub fn new(
        id: Uuid,
        engine: RuleEngine,
        players: [Option<Uuid>; 2],
        seed: [u8; 8],
        created_at: u64,
    ) -> Result<Self, OnlineError> {
        if id.is_nil()
            || players.iter().all(Option::is_none)
            || players.iter().flatten().any(Uuid::is_nil)
            || (players[0].is_some() && players[0] == players[1])
        {
            return Err(OnlineError::Malformed);
        }
        let visible = [Seat::One, Seat::Two]
            .map(|seat| Visible::new(from_projection(&engine.projection(seat), seat), created_at));
        Ok(Self {
            id,
            engine,
            players,
            seed,
            epochs: [1; 2],
            revisions: [0; 2],
            visible,
            changed: [false; 2],
            now: created_at,
            created_at,
            acknowledgements: [HashMap::new(), HashMap::new()],
        })
    }
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn players(&self) -> [Option<Uuid>; 2] {
        self.players
    }
    pub fn seat(&self, account: Uuid) -> Result<Seat, OnlineError> {
        match self.players.iter().position(|p| *p == Some(account)) {
            Some(0) => Ok(Seat::One),
            Some(1) => Ok(Seat::Two),
            _ => Err(OnlineError::NotMatched),
        }
    }
    pub fn attach(&mut self, seat: Seat, at: u64) -> Result<u32, OnlineError> {
        self.advance(at);
        let epoch = self.epochs[seat.index()]
            .checked_add(1)
            .ok_or(OnlineError::Unavailable)?;
        self.engine
            .resume(seat, epoch, at)
            .map_err(|_| OnlineError::InvalidEpoch)?;
        self.epochs[seat.index()] = epoch;
        self.now = at;
        self.synchronize();
        Ok(epoch)
    }
    pub fn view(&self, seat: Seat) -> GameView {
        let mut view = from_projection(&self.engine.projection(seat), seat);
        view.revision = u64::from(self.revisions[seat.index()]);
        view
    }
    pub fn apply(&mut self, seat: Seat, input: OnlineInput, at: u64) -> OnlinePayload {
        self.advance(at);
        if input.match_id != self.id.to_string() {
            return OnlinePayload::Error {
                code: OnlineError::WrongMatch,
            };
        }
        let Ok(id) = Uuid::parse_str(&input.command_id) else {
            return OnlinePayload::Error {
                code: OnlineError::Malformed,
            };
        };
        if id.is_nil() {
            return OnlinePayload::Error {
                code: OnlineError::Malformed,
            };
        }
        if input.known_revision > self.revisions[seat.index()] {
            return OnlinePayload::Ack {
                command_id: input.command_id,
                revision: self.revisions[seat.index()],
                status: AckStatus::Rejected,
                error: Some(PublicError::Stale),
                duplicate: false,
            };
        }
        let action = match input.action {
            PublicAction::Open { cell } => Action::Open(CellId(cell)),
            PublicAction::Flag { cell } => Action::ToggleFlag(CellId(cell)),
            PublicAction::Accuse { cell } => Action::Accuse(CellId(cell)),
            PublicAction::Attack => Action::Attack,
        };
        let ack = self.engine.apply(Command {
            id: id.as_u128(),
            seq: u64::from(input.client_seq),
            epoch: input.session_epoch,
            seat,
            received_at: at,
            action,
        });
        self.synchronize();
        let revision = if ack.duplicate {
            self.acknowledgements[seat.index()]
                .get(&id)
                .copied()
                .unwrap_or(self.revisions[seat.index()])
        } else {
            let revision = self.revisions[seat.index()];
            if !matches!(
                ack.status,
                ActionStatus::Rejected(
                    Rejection::InvalidTime
                        | Rejection::InvalidEpoch
                        | Rejection::InvalidSequence
                        | Rejection::CommandConflict
                        | Rejection::CommandLimit
                )
            ) {
                self.acknowledgements[seat.index()].insert(id, revision);
            }
            revision
        };
        let (status, error) = match ack.status {
            ActionStatus::Applied => (AckStatus::Applied, None),
            ActionStatus::Rejected(error) => (AckStatus::Rejected, Some(error.into())),
        };
        OnlinePayload::Ack {
            command_id: input.command_id,
            revision,
            status,
            error,
            duplicate: ack.duplicate,
        }
    }
    pub fn advance(&mut self, at: u64) {
        if self.engine.advance(at).is_ok() {
            self.now = at;
            self.synchronize();
        }
    }
    pub(super) fn start_deadline(&self) -> u64 {
        self.engine.start_ms()
    }
    pub(super) fn cancel_initial(&mut self, at: u64) {
        if self.engine.cancel_before_start(at).is_ok() {
            self.now = at;
            self.synchronize();
        } else {
            self.abort(self.now.max(at));
        }
    }
    pub fn disconnect(&mut self, seat: Seat, epoch: u32, at: u64) {
        if self.epochs[seat.index()] == epoch && self.engine.disconnect(seat, at).is_ok() {
            self.now = at;
            self.synchronize();
        }
    }
    pub fn proof_work(&self, seat: Seat) -> AnalysisJob {
        self.engine.proof_work(seat)
    }
    pub(super) fn bot_projection(&self, seat: Seat) -> liar_core::game::Projection {
        self.engine.projection(seat)
    }
    pub(super) fn apply_bot(
        &mut self,
        seat: Seat,
        intent: BotIntent,
        at: u64,
    ) -> Result<bool, OnlineError> {
        if self.players[seat.index()].is_some() {
            return Err(OnlineError::Unauthorized);
        }
        self.advance(at);
        let ack = self.engine.apply(Command {
            id: intent.id.as_u128(),
            seq: u64::from(intent.seq),
            epoch: self.epochs[seat.index()],
            seat,
            received_at: at,
            action: intent.action,
        });
        self.synchronize();
        Ok(ack.status == ActionStatus::Applied)
    }
    pub(super) fn abort(&mut self, at: u64) {
        if self.engine.abort(at).is_ok() {
            self.now = at;
            self.synchronize();
        }
    }
    pub fn commit_proof(&mut self, proof: AnalysisResult) -> bool {
        self.engine.commit_proof(proof).is_ok()
    }
    pub fn take_changes(&mut self) -> Vec<(Seat, GameView)> {
        let changed = std::mem::take(&mut self.changed);
        [Seat::One, Seat::Two]
            .into_iter()
            .filter(|seat| changed[seat.index()])
            .map(|seat| (seat, self.view(seat)))
            .collect()
    }
    fn synchronize(&mut self) {
        for seat in [Seat::One, Seat::Two] {
            let index = seat.index();
            let visible = Visible::new(
                from_projection(&self.engine.projection(seat), seat),
                self.now,
            );
            if self.visible[index] != visible {
                self.revisions[index] = self.revisions[index].saturating_add(1);
                self.visible[index] = visible;
                self.changed[index] = true;
            }
        }
    }
    pub fn finished(&self) -> Option<FinishedMatch> {
        let end = self.engine.projection(Seat::One).end?;
        let views = [Seat::One, Seat::Two].map(|seat| self.view(seat));
        Some(FinishedMatch {
            id: self.id,
            rules_hash: views[0].rules.hash.clone(),
            seed: self.seed,
            ended_ms: end.at.saturating_sub(self.created_at),
            reason: views[0].result.as_ref()?.reason,
            players: [0, 1].map(|i| PlayerResult {
                account: self.players[i],
                outcome: views[i]
                    .result
                    .as_ref()
                    .expect("Both projections have the same end")
                    .outcome,
                opened_safe: views[i]
                    .own
                    .cells
                    .iter()
                    .filter(|c| c.state == CellState::Safe)
                    .count() as u16,
                mistakes: views[i].own.stats.mistakes,
                accusations: views[i].own.stats.accusation_attempts,
                correct_accusations: views[i].own.stats.correct_accusations,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liar_core::{board::Board, rules::RulesSnapshot};

    #[test]
    fn bot_system_input_cannot_claim_human_authority_or_bypass_mine_stun() {
        let mut rules = RulesSnapshot::bundled().rules;
        rules.width = 3;
        rules.height = 3;
        rules.mines = 2;
        let rules = RulesSnapshot::from_rules(rules).unwrap();
        let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
        let mut state = MatchState::new(
            Uuid::new_v4(),
            RuleEngine::new(board, rules, 0).unwrap(),
            [Some(Uuid::new_v4()), None],
            [48; 8],
            0,
        )
        .unwrap();
        let intent = |seq, cell| BotIntent {
            id: Uuid::new_v4(),
            seq,
            action: Action::Open(CellId(cell)),
        };
        let human = state.view(Seat::One);
        assert_eq!(
            state.apply_bot(Seat::One, intent(1, 8), 3000),
            Err(OnlineError::Unauthorized)
        );
        assert_eq!(state.view(Seat::One), human);
        assert_eq!(state.apply_bot(Seat::Two, intent(1, 5), 3000), Ok(true));
        assert!(state.view(Seat::Two).own.stun_ms > 0);
        assert_eq!(state.apply_bot(Seat::Two, intent(2, 8), 3020), Ok(false));
        assert_eq!(state.view(Seat::Two).own.cells[8].number, None);
        assert_eq!(state.apply_bot(Seat::Two, intent(3, 8), 10000), Ok(true));
    }
}
