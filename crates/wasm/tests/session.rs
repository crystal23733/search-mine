use liar_protocol::game::{AckStatus, GamePhase, PublicError};
use liar_wasm::session::PracticeSession;
#[test]
fn practice_adapter_runs_shared_domain_and_deduplicates_flags() {
    let mut session = PracticeSession::new("42", "normal").unwrap();
    assert_eq!(session.view().phase, GamePhase::Countdown);
    assert_eq!(session.view().own.cells.len(), 256);
    let input = r#"{"v":1,"command_id":1,"client_seq":1,"action":{"type":"flag","cell":255}}"#;
    let first = session.step(input, 3000).unwrap();
    assert_eq!(first.ack.status, AckStatus::Applied);
    assert!(first.view.own.cells[255].flagged);
    let second = session.step(input, 3001).unwrap();
    assert!(second.ack.duplicate);
    assert!(second.view.own.cells[255].flagged);
    assert_eq!(session.step("{}", 3002).err(), Some(PublicError::Malformed));
    assert_eq!(session.advance(2999).err(), Some(PublicError::InvalidTime));
}

#[test]
fn adapter_rejects_invalid_configuration_and_inputs_without_mutating_state() {
    for seed in ["", "-1", "1.5", "18446744073709551616", "secret"] {
        assert!(matches!(
            PracticeSession::new(seed, "easy"),
            Err(PublicError::Malformed)
        ));
    }
    assert!(matches!(
        PracticeSession::new("42", "unknown"),
        Err(PublicError::Malformed)
    ));
    let mut session = PracticeSession::new("42", "easy").unwrap();
    let before = session.view();
    for input in [
        "{}",
        r#"{"v":1,"command_id":1,"client_seq":1,"action":{"type":"attack","target":0}}"#,
    ] {
        assert_eq!(
            session.step(input, 3000).err(),
            Some(PublicError::Malformed)
        );
        assert_eq!(session.view(), before);
    }
    assert_eq!(
        session.step(&" ".repeat(8193), 3000).err(),
        Some(PublicError::Malformed)
    );
    assert_eq!(session.view(), before);
    let before_start = session
        .step(
            r#"{"v":1,"command_id":1,"client_seq":1,"action":{"type":"attack"}}"#,
            0,
        )
        .unwrap();
    assert_eq!(before_start.ack.error, Some(PublicError::NotStarted));
    let finished = session.advance(243000).unwrap();
    assert_eq!(finished.phase, GamePhase::Finished);
    assert!(finished.result.unwrap().completed);
}
#[test]
fn js_clock_is_checked_before_integer_conversion() {
    use liar_wasm::session::checked_time_ms;
    for value in [-1.0, 1.5, f64::NAN, f64::INFINITY, 4294967296.0] {
        assert_eq!(checked_time_ms(value), Err(PublicError::InvalidTime));
    }
    assert_eq!(checked_time_ms(0.0), Ok(0));
    assert_eq!(checked_time_ms(4294967295.0), Ok(u32::MAX));
}
