//! Allowlisted public lesson state; the fixed teaching fixture has no online identity.
use crate::game::{GameView, LocalAck, PublicAction, from_projection};
use liar_core::{
    game::{Action, Seat},
    tutorial::{TutorialSession, TutorialStage},
};
use serde::Serialize;
use ts_rs::TS;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct TutorialView {
    pub stage: TutorialStage,
    pub expected: Option<PublicAction>,
    pub game: GameView,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct TutorialStep {
    pub ack: LocalAck,
    pub view: TutorialView,
}
pub fn from_tutorial(session: &TutorialSession) -> TutorialView {
    TutorialView {
        stage: session.stage(),
        expected: session.expected().map(|action| match action {
            Action::Open(cell) => PublicAction::Open { cell: cell.0 },
            Action::ToggleFlag(cell) => PublicAction::Flag { cell: cell.0 },
            Action::Accuse(cell) => PublicAction::Accuse { cell: cell.0 },
            Action::Attack => PublicAction::Attack,
        }),
        game: from_projection(&session.projection(), Seat::One),
    }
}
pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        TutorialStage::decl(config),
        TutorialView::decl(config),
        TutorialStep::decl(config),
    ]
}
