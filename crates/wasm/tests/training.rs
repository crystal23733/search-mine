use liar_core::tutorial::TutorialStage;
use liar_protocol::game::PublicError;
use liar_wasm::training::TrainingSession;
#[test]
fn native_training_adapter_returns_actual_public_steps_and_rejects_unknown_input_fields() {
    let mut session = TrainingSession::new().unwrap();
    let initial = session.view();
    assert_eq!(initial.stage, TutorialStage::Open);
    assert_eq!(session.step("{}", 0).err(), Some(PublicError::Malformed));
    assert_eq!(session.view(), initial);
    let inputs = [
        r#"{"type":"open","cell":8}"#,
        r#"{"type":"attack"}"#,
        r#"{"type":"open","cell":2}"#,
        r#"{"type":"accuse","cell":2}"#,
    ];
    for (index, action) in inputs.iter().enumerate() {
        let input = format!(
            r#"{{"v":1,"command_id":{},"client_seq":{},"action":{}}}"#,
            index + 1,
            index + 1,
            action
        );
        let result = session.step(&input, index as u32 + 1).unwrap();
        assert_eq!(result.ack.error, None);
        assert!(
            session
                .step(&input, index as u32 + 1)
                .unwrap()
                .ack
                .duplicate
        );
    }
    assert_eq!(session.view().stage, TutorialStage::Complete);
    assert_eq!(session.view().game.own.stats.correct_accusations, 1);
    let json = serde_json::to_string(&session.view()).unwrap();
    for forbidden in ["mine_cells", "overlay", "seed", "password", "truth"] {
        assert!(!json.contains(forbidden));
    }
    assert_eq!(session.advance(3).err(), Some(PublicError::InvalidTime));
    assert_eq!(session.advance(5).unwrap().game.opponent.stun_ms, 1999);
}

#[test]
fn wrong_lesson_actions_are_rejected_without_ending_training_or_reapplying_a_rejected_id() {
    let mut session = TrainingSession::new().unwrap();
    let wrong = r#"{"v":1,"command_id":1,"client_seq":1,"action":{"type":"attack"}}"#;
    let rejected = session.step(wrong, 1).unwrap();
    assert_eq!(rejected.ack.error, Some(PublicError::InvalidCell));
    assert_eq!(rejected.view.stage, TutorialStage::Open);
    let opening = r#"{"v":1,"command_id":2,"client_seq":2,"action":{"type":"open","cell":8}}"#;
    assert_eq!(
        session.step(opening, 2).unwrap().view.stage,
        TutorialStage::Attack
    );
    let repeated = session.step(wrong, 3).unwrap();
    assert!(repeated.ack.duplicate);
    assert_eq!(repeated.ack.error, Some(PublicError::InvalidCell));
    assert_eq!(repeated.view.stage, TutorialStage::Attack);
    assert_eq!(repeated.view.game.own.gauge, 1);
    let conflict = r#"{"v":1,"command_id":1,"client_seq":1,"action":{"type":"open","cell":8}}"#;
    assert_eq!(
        session.step(conflict, 4).unwrap().ack.error,
        Some(PublicError::Conflict)
    );
}
#[test]
fn rejected_lesson_receipts_are_bounded_and_malformed_frames_do_not_change_time() {
    let mut session = TrainingSession::new().unwrap();
    for serial in 1..=8192 {
        let input = format!(
            r#"{{"v":1,"command_id":{serial},"client_seq":{serial},"action":{{"type":"attack"}}}}"#
        );
        assert_eq!(
            session.step(&input, 0).unwrap().ack.error,
            Some(PublicError::InvalidCell)
        );
    }
    let overflow = r#"{"v":1,"command_id":8193,"client_seq":8193,"action":{"type":"attack"}}"#;
    assert_eq!(
        session.step(overflow, 0).err(),
        Some(PublicError::CommandLimit)
    );
    let before = session.view();
    assert_eq!(session.step("{}", 100).err(), Some(PublicError::Malformed));
    assert_eq!(session.view(), before);
}
