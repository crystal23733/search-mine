use super::*;
use crate::daily::{PublishedDaily, ReplayError, ReplayVerifier};
use liar_core::rules::RulesSnapshot;
use liar_protocol::daily::DailyReplay;

fn golden() -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap()["daily"]
        .clone()
}
fn definition() -> PublishedDaily {
    PublishedDaily::new("2026-10-07", 1, RulesSnapshot::bundled()).unwrap()
}
#[test]
fn verified_evidence_binds_the_canonical_payload_that_was_actually_replayed() {
    use sha2::Digest;
    let mut source = golden()["replay"].clone();
    let replay: DailyReplay = serde_json::from_value(source.clone()).unwrap();
    let canonical = serde_json::to_string(&replay).unwrap();
    let evidence = NativeReplayVerifier
        .verify(
            &definition(),
            &serde_json::to_string_pretty(&source).unwrap(),
        )
        .unwrap();
    assert_eq!(evidence.canonical_replay(), canonical);
    let digest: [u8; 32] = sha2::Sha256::digest(canonical.as_bytes()).into();
    assert_eq!(evidence.fingerprint(), digest);
    let same = NativeReplayVerifier
        .verify(&definition(), &source.to_string())
        .unwrap();
    assert_eq!(same, evidence);
    source["final_time_ms"] = serde_json::json!(u32::MAX);
    let changed = NativeReplayVerifier
        .verify(&definition(), &source.to_string())
        .unwrap();
    assert_ne!(changed.fingerprint(), evidence.fingerprint());
    assert_eq!(changed.outcome(), evidence.outcome());
    assert_eq!(evidence.canonical_replay(), canonical);
    assert!(!format!("{evidence:?}").contains("inputs"));
}
#[test]
fn native_replay_matches_the_actual_native_wasm_golden_completion() {
    let fixture = golden();
    let replay: DailyReplay = serde_json::from_value(fixture["replay"].clone()).unwrap();
    let evidence = NativeReplayVerifier
        .verify(&definition(), &fixture["replay"].to_string())
        .unwrap();
    let view = &fixture["events"].as_array().unwrap().last().unwrap()["expected"]["view"];
    assert_eq!(evidence.metadata(), &replay.metadata);
    assert_eq!(
        evidence.mistakes(),
        view["own"]["stats"]["mistakes"].as_u64().unwrap() as u16
    );
    assert_eq!(evidence.replayed_elapsed_ms(), 3200);
    assert_eq!(
        evidence.opened_safe(),
        view["own"]["cells"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|cell| cell["state"] == "safe")
            .count()
    );
}

#[test]
fn every_published_metadata_field_is_bound_before_replaying() {
    let initial = golden()["replay"].clone();
    for (field, value) in [
        ("date", serde_json::json!("2026-10-08")),
        ("seed", serde_json::json!("1")),
        ("seed_version", serde_json::json!(2)),
        ("mode", serde_json::json!("duel")),
        ("rules_hash", serde_json::json!("a".repeat(64))),
        ("solver_version", serde_json::json!(2)),
        ("rng_version", serde_json::json!(2)),
    ] {
        let mut replay = initial.clone();
        replay["metadata"][field] = value;
        assert_eq!(
            NativeReplayVerifier.verify(&definition(), &replay.to_string()),
            Err(ReplayError::MetadataMismatch),
            "{field}"
        );
    }
}

#[test]
fn strict_wire_versions_fields_numbers_and_byte_count_are_enforced() {
    let initial = golden()["replay"].clone();
    assert_eq!(
        NativeReplayVerifier.verify(&definition(), "{"),
        Err(ReplayError::Malformed)
    );
    for field in [
        "v",
        "input_v",
        "extra",
        "action_extra",
        "zero_id",
        "zero_seq",
        "negative_time",
    ] {
        let mut replay = initial.clone();
        let expected = match field {
            "v" => {
                replay["v"] = serde_json::json!(2);
                ReplayError::UnsupportedVersion
            }
            "input_v" => {
                replay["inputs"][0]["input"]["v"] = serde_json::json!(2);
                ReplayError::UnsupportedVersion
            }
            "extra" => {
                replay["stats"] = serde_json::json!({"mistakes":0});
                ReplayError::Malformed
            }
            "action_extra" => {
                replay["inputs"][0]["input"]["action"]["ignored"] = serde_json::json!(1);
                ReplayError::Malformed
            }
            "zero_id" => {
                replay["inputs"][0]["input"]["command_id"] = serde_json::json!(0);
                ReplayError::Malformed
            }
            "zero_seq" => {
                replay["inputs"][0]["input"]["client_seq"] = serde_json::json!(0);
                ReplayError::Malformed
            }
            _ => {
                replay["inputs"][0]["time_ms"] = serde_json::json!(-1);
                ReplayError::Malformed
            }
        };
        assert_eq!(
            NativeReplayVerifier.verify(&definition(), &replay.to_string()),
            Err(expected),
            "{field}"
        );
    }
    let mut padded = initial.to_string();
    padded.extend(std::iter::repeat_n(
        ' ',
        liar_protocol::daily::MAX_DAILY_REPLAY_BYTES - padded.len(),
    ));
    assert!(NativeReplayVerifier.verify(&definition(), &padded).is_ok());
    padded.push(' ');
    assert_eq!(
        NativeReplayVerifier.verify(&definition(), &padded),
        Err(ReplayError::Malformed)
    );
}

#[test]
fn mode_cell_time_and_command_identity_violations_do_not_become_completions() {
    let initial = golden()["replay"].clone();
    for variant in 0..7 {
        let mut replay = initial.clone();
        match variant {
            0 => replay["inputs"][0]["input"]["action"] = serde_json::json!({"type":"attack"}),
            1 => {
                replay["inputs"][0]["input"]["action"] =
                    serde_json::json!({"type":"accuse","cell":0})
            }
            2 => replay["inputs"][0]["input"]["action"]["cell"] = serde_json::json!(65535),
            3 => replay["inputs"][1]["time_ms"] = serde_json::json!(0),
            4 => replay["final_time_ms"] = serde_json::json!(0),
            5 => replay["inputs"][2]["input"]["client_seq"] = serde_json::json!(1),
            _ => replay["inputs"][2]["input"]["command_id"] = serde_json::json!(1),
        }
        assert_eq!(
            NativeReplayVerifier.verify(&definition(), &replay.to_string()),
            Err(ReplayError::InvalidReplay),
            "{variant}"
        );
    }
}

#[test]
fn exact_duplicate_nonconsecutive_sequence_and_post_finish_time_preserve_evidence() {
    let mut replay: DailyReplay = serde_json::from_value(golden()["replay"].clone()).unwrap();
    let expected = NativeReplayVerifier
        .verify(&definition(), &serde_json::to_string(&replay).unwrap())
        .unwrap();
    for input in &mut replay.inputs {
        input.input.client_seq *= 2;
    }
    replay.inputs.insert(1, replay.inputs[0].clone());
    replay.final_time_ms = u32::MAX;
    assert_eq!(
        NativeReplayVerifier
            .verify(&definition(), &serde_json::to_string(&replay).unwrap())
            .map(|verified| verified.outcome().clone()),
        Ok(expected.outcome().clone())
    );
}

#[test]
fn legitimate_rejected_clicks_are_replayed_without_fabricating_mistakes() {
    use liar_protocol::{
        daily::DailyTimedInput,
        game::{LocalInput, PublicAction},
    };
    let mut replay: DailyReplay = serde_json::from_value(golden()["replay"].clone()).unwrap();
    let expected = NativeReplayVerifier
        .verify(&definition(), &serde_json::to_string(&replay).unwrap())
        .unwrap();
    for input in &mut replay.inputs {
        input.input.client_seq += 5;
    }
    let mut prefix = Vec::new();
    for (index, (action, time_ms)) in [
        (PublicAction::Open { cell: 0 }, 0),
        (PublicAction::Flag { cell: 0 }, 3000),
        (PublicAction::Flag { cell: 7 }, 3000),
        (PublicAction::Open { cell: 7 }, 3000),
        (PublicAction::Flag { cell: 7 }, 3000),
    ]
    .into_iter()
    .enumerate()
    {
        prefix.push(DailyTimedInput {
            input: LocalInput {
                v: 1,
                command_id: 1000 + index as u32,
                client_seq: 1 + index as u32,
                action,
            },
            time_ms,
        });
    }
    prefix.extend(replay.inputs);
    prefix.push(DailyTimedInput {
        input: LocalInput {
            v: 1,
            command_id: 2000,
            client_seq: 2000,
            action: PublicAction::Open { cell: 0 },
        },
        time_ms: 6300,
    });
    replay.inputs = prefix;
    replay.final_time_ms = 6300;
    assert_eq!(
        NativeReplayVerifier
            .verify(&definition(), &serde_json::to_string(&replay).unwrap())
            .map(|verified| verified.outcome().clone()),
        Ok(expected.outcome().clone())
    );
}

#[test]
fn unsupported_definition_and_wire_or_published_command_limits_are_explicit() {
    assert!(PublishedDaily::new("2026-02-30", 1, RulesSnapshot::bundled()).is_err());
    assert!(PublishedDaily::new("2026-10-07", 2, RulesSnapshot::bundled()).is_err());
    let mut invalid = RulesSnapshot::bundled();
    invalid.hash = "a".repeat(64);
    assert!(PublishedDaily::new("2026-10-07", 1, invalid).is_err());
    let mut rules = RulesSnapshot::bundled().rules;
    rules.max_commands_per_seat += 1;
    assert!(
        PublishedDaily::new("2026-10-07", 1, RulesSnapshot::from_rules(rules).unwrap()).is_err()
    );
    let mut replay = golden()["replay"].clone();
    replay["inputs"] = serde_json::json!(vec![replay["inputs"][0].clone(); 8193]);
    assert_eq!(
        NativeReplayVerifier.verify(&definition(), &replay.to_string()),
        Err(ReplayError::InvalidReplay)
    );
    let mut rules = RulesSnapshot::bundled().rules;
    rules.max_commands_per_seat = 1;
    let small =
        PublishedDaily::new("2026-10-07", 1, RulesSnapshot::from_rules(rules).unwrap()).unwrap();
    let mut replay = golden()["replay"].clone();
    replay["metadata"] = serde_json::to_value(small.metadata()).unwrap();
    assert_eq!(
        NativeReplayVerifier.verify(&small, &replay.to_string()),
        Err(ReplayError::InvalidReplay)
    );
}

#[test]
fn retained_rules_are_used_instead_of_current_bundled_rules_and_stunned_clicks_remain_valid() {
    use liar_core::{
        board::{CellId, ObservedCell},
        daily::DailyPuzzle,
        game::{Action, Command, Seat},
    };
    use liar_protocol::{
        daily::DailyTimedInput,
        game::{LocalInput, PublicAction},
    };
    let mut rules = RulesSnapshot::bundled().rules;
    rules.duration_ms += 1;
    let published =
        PublishedDaily::new("2026-10-07", 1, RulesSnapshot::from_rules(rules).unwrap()).unwrap();
    assert_ne!(
        published.metadata().rules_hash,
        definition().metadata().rules_hash
    );
    let mut puzzle =
        DailyPuzzle::with_rules(&published.metadata().date, 1, published.rules().clone()).unwrap();
    let mut replay = DailyReplay {
        v: 1,
        metadata: published.metadata().clone(),
        inputs: vec![],
        final_time_ms: 3000,
    };
    let mut time = 3000;
    let mut seq = 0u32;
    puzzle.advance(time).unwrap();
    let mut stunned = false;
    for cell in 0..256 {
        if puzzle.complete() {
            break;
        }
        let before = puzzle.projection();
        if before.own.cells.cell(CellId(cell)) != Some(ObservedCell::Unknown) {
            continue;
        }
        time += u64::from(before.own.stun_ms).max(50);
        seq += 1;
        puzzle.apply(Command {
            id: u128::from(seq),
            seq: u64::from(seq),
            epoch: 1,
            seat: Seat::One,
            received_at: time,
            action: Action::Open(CellId(cell)),
        });
        replay.inputs.push(DailyTimedInput {
            input: LocalInput {
                v: 1,
                command_id: seq,
                client_seq: seq,
                action: PublicAction::Open { cell },
            },
            time_ms: time as u32,
        });
        if puzzle.projection().own.stun_ms > 0 {
            seq += 1;
            puzzle.apply(Command {
                id: u128::from(seq),
                seq: u64::from(seq),
                epoch: 1,
                seat: Seat::One,
                received_at: time,
                action: Action::Open(CellId(0)),
            });
            replay.inputs.push(DailyTimedInput {
                input: LocalInput {
                    v: 1,
                    command_id: seq,
                    client_seq: seq,
                    action: PublicAction::Open { cell: 0 },
                },
                time_ms: time as u32,
            });
            stunned = true;
        }
    }
    assert!(stunned);
    assert!(puzzle.complete());
    replay.final_time_ms = time as u32;
    let source = serde_json::to_string(&replay).unwrap();
    let verified = NativeReplayVerifier.verify(&published, &source).unwrap();
    assert_eq!(verified.metadata(), published.metadata());
    assert_eq!(verified.mistakes(), puzzle.projection().own.stats.mistakes);
    assert!(verified.mistakes() > 0);
    assert_eq!(verified.opened_safe(), 216);
    assert_eq!(
        NativeReplayVerifier.verify(&definition(), &source),
        Err(ReplayError::MetadataMismatch)
    );
}
#[test]
fn unfinished_and_timeout_replays_are_not_completion_evidence() {
    let mut replay = golden()["replay"].clone();
    replay["inputs"] = serde_json::json!([]);
    for time in [0, 3000, 243000, u32::MAX] {
        replay["final_time_ms"] = serde_json::json!(time);
        assert_eq!(
            NativeReplayVerifier.verify(&definition(), &replay.to_string()),
            Err(ReplayError::Incomplete)
        );
    }
}
