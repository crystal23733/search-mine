use liar_protocol::game::{CellState, PublicError};
use liar_wasm::daily::DailySession;
#[test]
fn daily_adapter_exports_only_own_public_data_and_replays_duplicates_flags_and_rejections() {
    let mut session = DailySession::new("2026-10-07", 1).unwrap();
    let initial = session.view();
    let value = serde_json::to_value(&initial).unwrap();
    assert!(value.get("opponent").is_none());
    assert_eq!(
        session.step("{}", 10000).err(),
        Some(PublicError::Malformed)
    );
    assert_eq!(session.view(), initial);
    let cell = initial
        .own
        .cells
        .iter()
        .find(|c| c.state == CellState::Closed)
        .unwrap()
        .cell;
    let flag = format!(
        r#"{{"v":1,"command_id":1,"client_seq":1,"action":{{"type":"flag","cell":{cell}}}}}"#
    );
    let flagged = session.step(&flag, 3000).unwrap();
    assert!(flagged.view.own.cells[usize::from(cell)].flagged);
    assert!(session.step(&flag, 3001).unwrap().ack.duplicate);
    let attack = r#"{"v":1,"command_id":2,"client_seq":2,"action":{"type":"attack"}}"#;
    assert_eq!(
        session.step(attack, 4000).unwrap().ack.error,
        Some(PublicError::Unavailable)
    );
    session.advance(5000).unwrap();
    let before = session.view();
    assert_eq!(session.advance(4000).err(), Some(PublicError::InvalidTime));
    assert_eq!(session.view(), before);
    let replay = session.export_replay();
    assert_eq!(replay.inputs.len(), 3);
    let source = serde_json::to_string(&replay).unwrap();
    let restored = DailySession::from_replay(&source).unwrap();
    assert_eq!(restored.view(), session.view());
    assert_eq!(restored.export_replay(), replay);
    let mut modified = replay;
    modified.metadata.seed = "forged".into();
    assert_eq!(
        DailySession::from_replay(&serde_json::to_string(&modified).unwrap()).err(),
        Some(PublicError::Malformed)
    );
}
#[test]
fn daily_replay_rejects_extra_fields_time_reversal_versions_and_oversized_logs() {
    use liar_protocol::daily::{DailyTimedInput, MAX_DAILY_REPLAY_BYTES};
    use liar_wasm::daily::checked_seed_version;
    for version in [-1.0, 0.0, 1.5, 2.0, 65537.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            checked_seed_version(version),
            Err(PublicError::UnsupportedVersion)
        );
    }
    assert_eq!(checked_seed_version(1.0), Ok(1));
    assert_eq!(
        DailySession::new("2026-02-30", 1).err(),
        Some(PublicError::Malformed)
    );
    let mut session = DailySession::new("2026-10-07", 1).unwrap();
    let input = r#"{"v":1,"command_id":1,"client_seq":1,"action":{"type":"attack"}}"#;
    session.step(input, 3000).unwrap();
    let valid = serde_json::to_value(session.export_replay()).unwrap();
    for (pointer, value, expected) in [
        ("/v", serde_json::json!(99), PublicError::UnsupportedVersion),
        (
            "/metadata/seed_version",
            serde_json::json!(2),
            PublicError::UnsupportedVersion,
        ),
        (
            "/metadata/mode",
            serde_json::json!("duel"),
            PublicError::Malformed,
        ),
        (
            "/final_time_ms",
            serde_json::json!(2000),
            PublicError::InvalidTime,
        ),
        (
            "/inputs/0/input/command_id",
            serde_json::json!(0),
            PublicError::Malformed,
        ),
    ] {
        let mut changed = valid.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert_eq!(
            DailySession::from_replay(&changed.to_string()).err(),
            Some(expected)
        );
    }
    let mut extra = valid.clone();
    extra["inputs"][0]["input"]["action"]["cell"] = serde_json::json!(10);
    assert_eq!(
        DailySession::from_replay(&extra.to_string()).err(),
        Some(PublicError::Malformed)
    );
    assert_eq!(
        DailySession::from_replay(&" ".repeat(MAX_DAILY_REPLAY_BYTES + 1)).err(),
        Some(PublicError::Malformed)
    );
    let mut replay = session.export_replay();
    let entry: DailyTimedInput = replay.inputs[0].clone();
    let limit = usize::from(session.view().rules.rules.max_commands_per_seat);
    replay.inputs = vec![entry.clone(); limit + 1];
    assert_eq!(
        DailySession::from_replay(&serde_json::to_string(&replay).unwrap()).err(),
        Some(PublicError::CommandLimit)
    );
    for _ in 1..limit {
        assert!(session.step(input, 3000).unwrap().ack.duplicate);
    }
    assert_eq!(
        session.step(input, 3000).err(),
        Some(PublicError::CommandLimit)
    );
    assert_eq!(session.export_replay().inputs.len(), limit);
}
