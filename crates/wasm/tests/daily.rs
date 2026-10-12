use liar_core::rules::RulesSnapshot;
use liar_protocol::game::{CellState, PublicError};
use liar_wasm::daily::DailySession;

fn accepted_fixture() -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(include_str!(
        "../../../tests/fixtures/native-wasm.json"
    ))
    .unwrap()["accepted_daily"]
        .clone()
}

#[test]
fn accepted_snapshot_matches_core_golden_for_all_steps_and_restores_only_with_the_same_rules() {
    let fixture = accepted_fixture();
    let snapshot: RulesSnapshot =
        serde_json::from_value(fixture["initial"]["rules"].clone()).unwrap();
    let mut session = DailySession::with_rules("2026-10-07", 1, snapshot.clone()).unwrap();
    assert_ne!(
        session.view().metadata,
        DailySession::new("2026-10-07", 1).unwrap().view().metadata
    );
    assert_eq!(
        serde_json::to_value(session.view()).unwrap(),
        fixture["initial"]
    );
    for event in fixture["events"].as_array().unwrap() {
        let step = session
            .step(
                event["input"].as_str().unwrap(),
                event["time_ms"].as_u64().unwrap().try_into().unwrap(),
            )
            .unwrap();
        assert_eq!(serde_json::to_value(step).unwrap(), event["expected"]);
    }
    assert!(session.view().completed);
    assert_eq!(
        serde_json::to_value(session.export_replay()).unwrap(),
        fixture["replay"]
    );
    let source = fixture["replay"].to_string();
    let restored = DailySession::from_replay_with_rules(&source, snapshot).unwrap();
    assert_eq!(restored.view(), session.view());
    assert_eq!(restored.export_replay(), session.export_replay());
    assert_eq!(
        DailySession::from_replay(&source).err(),
        Some(PublicError::Malformed)
    );
    let mut forged = fixture["replay"].clone();
    forged["metadata"]["seed"] = serde_json::json!("forged");
    let rules: RulesSnapshot = serde_json::from_value(fixture["initial"]["rules"].clone()).unwrap();
    assert_eq!(
        DailySession::from_replay_with_rules(&forged.to_string(), rules).err(),
        Some(PublicError::Malformed)
    );
}

#[test]
fn accepted_json_is_closed_byte_bounded_verified_and_never_falls_back_to_bundled_rules() {
    use liar_wasm::daily::MAX_DAILY_RULES_BYTES;
    let rules = accepted_fixture()["initial"]["rules"].clone();
    let source = rules.to_string();
    let exact = source.clone() + &" ".repeat(MAX_DAILY_RULES_BYTES - source.len());
    assert_eq!(exact.len(), MAX_DAILY_RULES_BYTES);
    assert_eq!(
        DailySession::from_rules_json("2026-10-07", 1, &exact)
            .unwrap()
            .view()
            .countdown_ms,
        5000
    );
    let mut invalid = vec![
        "{".into(),
        "null".into(),
        exact + " ",
        "한".repeat(MAX_DAILY_RULES_BYTES / 2),
        source.replacen("{", "{\"hash\":\"duplicate\",", 1),
    ];
    for (pointer, value) in [
        ("/hash", serde_json::json!("forged")),
        ("/solver_version", serde_json::json!(999)),
        ("/rng_version", serde_json::json!(999)),
        ("/rules/countdown_ms", serde_json::json!(0)),
        ("/rules/width", serde_json::json!(0)),
    ] {
        let mut modified = rules.clone();
        *modified.pointer_mut(pointer).unwrap() = value;
        invalid.push(modified.to_string());
    }
    let mut extra = rules.clone();
    extra["extra"] = serde_json::json!(true);
    invalid.push(extra.to_string());
    let mut nested = rules.clone();
    nested["rules"]["extra"] = serde_json::json!(true);
    invalid.push(nested.to_string());
    for input in invalid {
        assert_eq!(
            DailySession::from_rules_json("2026-10-07", 1, &input).err(),
            Some(PublicError::Malformed)
        );
    }
    assert_eq!(
        DailySession::from_rules_json("2026-10-07", 2, &source).err(),
        Some(PublicError::UnsupportedVersion)
    );
    assert_eq!(
        DailySession::from_rules_json("2026-02-30", 1, &source).err(),
        Some(PublicError::Malformed)
    );
    let mut modified: RulesSnapshot = serde_json::from_value(rules).unwrap();
    modified.hash = "forged".into();
    assert_eq!(
        DailySession::with_rules("2026-10-07", 1, modified).err(),
        Some(PublicError::Malformed)
    );
    assert_eq!(
        DailySession::new("2026-10-07", 1)
            .unwrap()
            .view()
            .countdown_ms,
        3000
    );
}

#[test]
fn accepted_command_limits_and_the_existing_wire_replay_limit_both_remain_effective() {
    use liar_protocol::daily::DailyTimedInput;
    let mut rules = RulesSnapshot::bundled().rules;
    rules.max_commands_per_seat = 2;
    let snapshot = RulesSnapshot::from_rules(rules.clone()).unwrap();
    let mut session = DailySession::with_rules("2026-10-07", 1, snapshot.clone()).unwrap();
    let input = r#"{"v":1,"command_id":1,"client_seq":1,"action":{"type":"attack"}}"#;
    session.step(input, 3000).unwrap();
    session.step(input, 3000).unwrap();
    assert_eq!(
        session.step(input, 3000).err(),
        Some(PublicError::CommandLimit)
    );
    let mut replay = session.export_replay();
    replay.inputs.push(replay.inputs[0].clone());
    assert_eq!(
        DailySession::from_replay_with_rules(&serde_json::to_string(&replay).unwrap(), snapshot)
            .err(),
        Some(PublicError::CommandLimit)
    );
    rules.max_commands_per_seat = 9000;
    let snapshot = RulesSnapshot::from_rules(rules).unwrap();
    let session = DailySession::with_rules("2026-10-07", 1, snapshot.clone()).unwrap();
    let mut replay = session.export_replay();
    replay.inputs = vec![
        DailyTimedInput {
            input: liar_protocol::game::decode_local(input).unwrap(),
            time_ms: 3000
        };
        8193
    ];
    replay.final_time_ms = 3000;
    assert_eq!(
        DailySession::from_replay_with_rules(&serde_json::to_string(&replay).unwrap(), snapshot)
            .err(),
        Some(PublicError::CommandLimit)
    );
}

#[test]
fn exported_daily_browser_adapter_serializes_the_same_bundled_and_accepted_public_contract() {
    let fixture = accepted_fixture();
    let mut bundled = liar_wasm::DailySession::new("2026-10-07", 1.0).unwrap();
    let initial: serde_json::Value = serde_json::from_str(&bundled.snapshot().unwrap()).unwrap();
    assert_eq!(
        initial,
        serde_json::to_value(DailySession::new("2026-10-07", 1).unwrap().view()).unwrap()
    );
    let running: serde_json::Value =
        serde_json::from_str(&bundled.advance(3000.0).unwrap()).unwrap();
    assert_eq!(running["phase"], "playing");
    let mut accepted = liar_wasm::DailySession::with_rules(
        "2026-10-07",
        1.0,
        &fixture["initial"]["rules"].to_string(),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&accepted.snapshot().unwrap()).unwrap(),
        fixture["initial"]
    );
    for event in fixture["events"].as_array().unwrap() {
        let step = accepted
            .step(
                event["input"].as_str().unwrap(),
                event["time_ms"].as_f64().unwrap(),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&step).unwrap(),
            event["expected"]
        );
    }
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&accepted.replay().unwrap()).unwrap(),
        fixture["replay"]
    );
}
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
