//! Guided public training fixture using the same game engine as normal matches.
use crate::{
    board::{Board, CellId},
    game::{Ack, Action, ActionStatus, Command, Projection, Rejection, RuleEngine, Seat},
    rules::RulesSnapshot,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum TutorialStage {
    Open,
    Attack,
    Reveal,
    Accuse,
    Complete,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    width: u8,
    height: u8,
    mine_cells: Vec<u16>,
    first_open: u16,
    reveal_cell: u16,
    gauge_capacity: u16,
    countdown_ms: u32,
}
pub struct TutorialSession {
    engine: RuleEngine,
    stage: TutorialStage,
    settings: Settings,
    accepted: BTreeSet<u128>,
}
impl TutorialSession {
    pub fn new() -> Result<Self, Rejection> {
        let settings: Settings = toml::from_str(include_str!("../../../config/tutorial.toml"))
            .map_err(|_| Rejection::InvalidRules)?;
        let mut rules = RulesSnapshot::bundled().rules;
        rules.width = settings.width;
        rules.height = settings.height;
        rules.mines = settings
            .mine_cells
            .len()
            .try_into()
            .map_err(|_| Rejection::InvalidRules)?;
        rules.gauge_capacity = settings.gauge_capacity;
        rules.countdown_ms = settings.countdown_ms;
        let snapshot = RulesSnapshot::from_rules(rules).map_err(|_| Rejection::InvalidRules)?;
        let board = Board::from_mines(
            snapshot.rules.board_spec(),
            &settings
                .mine_cells
                .iter()
                .copied()
                .map(CellId)
                .collect::<Vec<_>>(),
        )
        .map_err(|_| Rejection::InvalidBoard)?;
        let mut engine = RuleEngine::new(board, snapshot, 0)?;
        let preparation = engine.apply(Command {
            id: 1,
            seq: 1,
            epoch: 1,
            seat: Seat::Two,
            received_at: 0,
            action: Action::Open(CellId(settings.first_open)),
        });
        if preparation.status != ActionStatus::Applied {
            return Err(Rejection::InvalidBoard);
        }
        let mut lesson = Self {
            engine,
            stage: TutorialStage::Open,
            settings,
            accepted: BTreeSet::new(),
        };
        lesson.refresh()?;
        Ok(lesson)
    }
    pub fn stage(&self) -> TutorialStage {
        self.stage
    }
    pub fn expected(&self) -> Option<Action> {
        match self.stage {
            TutorialStage::Open => Some(Action::Open(CellId(self.settings.first_open))),
            TutorialStage::Attack => Some(Action::Attack),
            TutorialStage::Reveal => Some(Action::Open(CellId(self.settings.reveal_cell))),
            TutorialStage::Accuse => Some(Action::Accuse(CellId(self.settings.reveal_cell))),
            TutorialStage::Complete => None,
        }
    }
    pub fn projection(&self) -> Projection {
        self.engine.projection(Seat::One)
    }
    pub fn apply(&mut self, command: Command) -> Result<Ack, Rejection> {
        if command.seat != Seat::One {
            return Err(Rejection::InvalidCell);
        }
        if self.accepted.contains(&command.id) {
            return Ok(self.engine.apply(command));
        }
        if self.expected() != Some(command.action) {
            return Err(Rejection::InvalidCell);
        }
        let ack = self.engine.apply(command);
        if ack.status != ActionStatus::Applied || ack.duplicate {
            return Ok(ack);
        }
        self.accepted.insert(command.id);
        self.refresh()?;
        self.stage = match self.stage {
            TutorialStage::Open => TutorialStage::Attack,
            TutorialStage::Attack => {
                let response = self.engine.apply(Command {
                    id: 2,
                    seq: 2,
                    epoch: 1,
                    seat: Seat::Two,
                    received_at: command.received_at,
                    action: Action::Attack,
                });
                if response.status != ActionStatus::Applied {
                    return Err(Rejection::InvalidBoard);
                }
                TutorialStage::Reveal
            }
            TutorialStage::Reveal => TutorialStage::Accuse,
            TutorialStage::Accuse | TutorialStage::Complete => TutorialStage::Complete,
        };
        Ok(ack)
    }
    pub fn advance(&mut self, now: u64) -> Result<(), Rejection> {
        self.engine.advance(now)
    }
    fn refresh(&mut self) -> Result<(), Rejection> {
        for seat in [Seat::One, Seat::Two] {
            let proof = self.engine.proof_work(seat).run()?;
            self.engine.commit_proof(proof)?;
        }
        Ok(())
    }
}
