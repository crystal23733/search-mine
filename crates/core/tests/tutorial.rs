use liar_core::{
    board::{CellId, ObservedCell},
    game::{Action, ActionStatus, Command, Rejection, Seat},
    tutorial::{TutorialSession, TutorialStage},
};
fn command(id: u64, action: Action) -> Command {
    Command {
        id: u128::from(id),
        seq: id,
        epoch: 1,
        seat: Seat::One,
        received_at: id,
        action,
    }
}
#[test]
fn lesson_uses_real_attack_accusation_and_reflection_once_with_duplicate_acks() {
    let mut lesson = TutorialSession::new().unwrap();
    assert_eq!(lesson.stage(), TutorialStage::Open);
    assert_eq!(lesson.expected(), Some(Action::Open(CellId(8))));
    assert_eq!(lesson.projection().rules.rules.gauge_capacity, 1);
    assert_eq!(lesson.projection().own.gauge, 0);
    let open = command(1, Action::Open(CellId(8)));
    let ack = lesson.apply(open).unwrap();
    assert_eq!(ack.status, ActionStatus::Applied);
    assert!(lesson.apply(open).unwrap().duplicate);
    assert_eq!(lesson.stage(), TutorialStage::Attack);
    assert_eq!(lesson.projection().own.gauge, 1);
    let attack = command(2, Action::Attack);
    lesson.apply(attack).unwrap();
    assert!(lesson.apply(attack).unwrap().duplicate);
    assert_eq!(lesson.stage(), TutorialStage::Reveal);
    assert_eq!(lesson.projection().own.gauge, 0);
    lesson.apply(command(3, Action::Open(CellId(2)))).unwrap();
    assert_eq!(lesson.stage(), TutorialStage::Accuse);
    assert_eq!(
        lesson.projection().own.cells.cell(CellId(2)),
        Some(ObservedCell::Number(2))
    );
    let accuse = command(4, Action::Accuse(CellId(2)));
    let ack = lesson.apply(accuse).unwrap();
    assert_eq!(ack.status, ActionStatus::Applied);
    assert!(lesson.apply(accuse).unwrap().duplicate);
    assert_eq!(lesson.stage(), TutorialStage::Complete);
    assert_eq!(lesson.expected(), None);
    assert_eq!(lesson.projection().own.stats.correct_accusations, 1);
    assert_eq!(
        lesson.projection().own.cells.cell(CellId(2)),
        Some(ObservedCell::Number(1))
    );
    assert_eq!(lesson.projection().opponent.stun_ms, 2000);
    assert_eq!(lesson.projection().end, None);
    assert_eq!(
        lesson.apply(command(5, Action::Attack)),
        Err(Rejection::InvalidCell)
    );
}
#[test]
fn out_of_step_wrong_seat_invalid_time_and_conflicts_never_advance_the_lesson() {
    let mut lesson = TutorialSession::new().unwrap();
    assert_eq!(
        lesson.apply(command(1, Action::Attack)),
        Err(Rejection::InvalidCell)
    );
    assert_eq!(lesson.stage(), TutorialStage::Open);
    assert_eq!(
        lesson.apply(Command {
            seat: Seat::Two,
            ..command(1, Action::Open(CellId(8)))
        }),
        Err(Rejection::InvalidCell)
    );
    lesson.apply(command(1, Action::Open(CellId(8)))).unwrap();
    assert_eq!(
        lesson.apply(command(1, Action::Attack)).unwrap().status,
        ActionStatus::Rejected(Rejection::CommandConflict)
    );
    lesson.advance(10).unwrap();
    assert_eq!(lesson.advance(9), Err(Rejection::InvalidTime));
    assert_eq!(lesson.stage(), TutorialStage::Attack);
    assert_eq!(
        lesson.apply(command(2, Action::Attack)).unwrap().status,
        ActionStatus::Rejected(Rejection::InvalidTime)
    );
    assert_eq!(lesson.stage(), TutorialStage::Attack);
}
