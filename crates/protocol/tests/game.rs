use liar_protocol::game::{PublicAction, PublicError, decode_local};
#[test]
fn input_decoder_accepts_public_intent_and_rejects_extra_target_or_invalid_frames() {
    let valid = r#"{"v":1,"command_id":1,"client_seq":1,"action":{"type":"open","cell":8}}"#;
    assert_eq!(
        decode_local(valid).unwrap().action,
        PublicAction::Open { cell: 8 }
    );
    for invalid in [
        valid.replace("\"cell\":8", "\"cell\":-1"),
        valid.replace("\"cell\":8", "\"cell\":65536"),
        valid.replace("\"open\"", "\"cheat\""),
        valid.replace("\"command_id\":1", "\"command_id\":0"),
        valid.replace("\"client_seq\":1", "\"client_seq\":0"),
        valid.replace("\"cell\":8", "\"cell\":8,\"truth\":1"),
        r#"{"v":1,"command_id":1,"client_seq":1,"action":{"type":"attack","cell":8}}"#.into(),
    ] {
        assert_eq!(decode_local(&invalid), Err(PublicError::Malformed));
    }
    assert_eq!(
        decode_local(&valid.replace("\"v\":1", "\"v\":99")),
        Err(PublicError::UnsupportedVersion)
    );
    assert_eq!(decode_local(&"x".repeat(8193)), Err(PublicError::Malformed));
}

fn engine() -> liar_core::game::RuleEngine {
    use liar_core::{
        board::{Board, CellId},
        game::RuleEngine,
        rules::RulesSnapshot,
    };
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    rules.gauge_capacity = 1;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    RuleEngine::new(
        Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap(),
        rules,
        0,
    )
    .unwrap()
}
#[test]
fn serialized_projection_contains_only_own_display_and_public_opponent_progress() {
    use liar_core::{
        board::CellId,
        game::{Action, Command, Seat},
    };
    use liar_protocol::game::from_projection;
    let mut engine = engine();
    for seat in [Seat::One, Seat::Two] {
        engine.apply(Command {
            id: 1,
            seq: 1,
            epoch: 1,
            seat,
            received_at: 3000,
            action: Action::Open(CellId(8)),
        });
    }
    engine
        .commit_proof(engine.proof_work(Seat::Two).run().unwrap())
        .unwrap();
    let before =
        serde_json::to_value(from_projection(&engine.projection(Seat::Two), Seat::Two)).unwrap();
    engine.apply(Command {
        id: 2,
        seq: 2,
        epoch: 1,
        seat: Seat::One,
        received_at: 3000,
        action: Action::Attack,
    });
    assert_eq!(
        serde_json::to_value(from_projection(&engine.projection(Seat::Two), Seat::Two)).unwrap(),
        before
    );
    engine.apply(Command {
        id: 2,
        seq: 2,
        epoch: 1,
        seat: Seat::Two,
        received_at: 3000,
        action: Action::Open(CellId(2)),
    });
    let json =
        serde_json::to_value(from_projection(&engine.projection(Seat::Two), Seat::Two)).unwrap();
    assert_eq!(json["own"]["cells"][2]["number"], 2);
    assert_eq!(json["own"]["cells"][5]["number"], serde_json::Value::Null);
    assert_eq!(
        json["opponent"],
        serde_json::json!({"opened_safe":5,"stun_ms":0,"reconnect_ms":null})
    );
    fn audit(value: &serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    assert!(
                        ![
                            "seed",
                            "board_hash",
                            "truth",
                            "source",
                            "overlay",
                            "active_lies",
                            "lie",
                            "is_lie",
                            "opponent_cells",
                            "registered"
                        ]
                        .contains(&key.as_str()),
                        "{key}"
                    );
                    audit(child);
                }
            }
            serde_json::Value::Array(array) => {
                for child in array {
                    audit(child);
                }
            }
            _ => {}
        }
    }
    audit(&json);
}

#[test]
fn public_grace_follows_the_detected_disconnect_and_keeps_zero_pending_until_both_expire() {
    use liar_core::game::Seat;
    use liar_protocol::game::from_projection;
    let mut g = engine();
    let peer = |g: &liar_core::game::RuleEngine, seat| {
        serde_json::to_value(from_projection(&g.projection(seat), seat)).unwrap()
    };
    assert_eq!(
        peer(&g, Seat::Two)["opponent"]["reconnect_ms"],
        serde_json::Value::Null
    );
    g.disconnect(Seat::One, 3000).unwrap();
    assert_eq!(peer(&g, Seat::Two)["opponent"]["reconnect_ms"], 30000);
    g.disconnect(Seat::One, 3500).unwrap();
    assert_eq!(peer(&g, Seat::Two)["opponent"]["reconnect_ms"], 29500);
    assert_eq!(
        peer(&g, Seat::One)["opponent"]["reconnect_ms"],
        serde_json::Value::Null
    );
    g.resume(Seat::One, 2, 4000).unwrap();
    assert_eq!(
        peer(&g, Seat::Two)["opponent"]["reconnect_ms"],
        serde_json::Value::Null
    );
    g.disconnect(Seat::One, 5000).unwrap();
    g.disconnect(Seat::Two, 6000).unwrap();
    g.advance(35000).unwrap();
    let waiting = peer(&g, Seat::Two);
    assert_eq!(waiting["opponent"]["reconnect_ms"], 0);
    assert_eq!(waiting["result"], serde_json::Value::Null);
    g.advance(35001).unwrap();
    assert_eq!(peer(&g, Seat::Two)["opponent"]["reconnect_ms"], 0);
    g.advance(36000).unwrap();
    let ended = peer(&g, Seat::Two);
    assert_eq!(ended["opponent"]["reconnect_ms"], serde_json::Value::Null);
    assert_eq!(ended["result"]["reason"], "abandoned");
    assert_eq!(ended["result"]["outcome"], "draw");
}
#[test]
fn public_results_distinguish_abort_cancel_completion_and_both_viewer_outcomes() {
    use liar_core::game::{EndReason, MatchEnd, Seat};
    use liar_protocol::game::{GamePhase, Outcome, from_projection};
    let mut projection = engine().projection(Seat::One);
    for reason in [
        EndReason::Clear,
        EndReason::Timeout,
        EndReason::Forfeit,
        EndReason::Abandoned,
        EndReason::ServerFailure,
        EndReason::Cancelled,
    ] {
        projection.end = Some(MatchEnd {
            reason,
            winner: Some(Seat::One),
            at: 3000,
        });
        let own = from_projection(&projection, Seat::One);
        let other = from_projection(&projection, Seat::Two);
        match reason {
            EndReason::ServerFailure => {
                assert_eq!(own.phase, GamePhase::Aborted);
                assert!(!own.result.unwrap().completed);
            }
            EndReason::Cancelled => {
                assert_eq!(own.phase, GamePhase::Cancelled);
                assert!(!own.result.unwrap().completed);
            }
            _ => {
                assert_eq!(own.result.unwrap().outcome, Outcome::Win);
                assert_eq!(other.result.unwrap().outcome, Outcome::Loss);
            }
        }
    }
    projection.end = Some(MatchEnd {
        reason: EndReason::Timeout,
        winner: None,
        at: 243000,
    });
    assert_eq!(
        from_projection(&projection, Seat::One)
            .result
            .unwrap()
            .outcome,
        Outcome::Draw
    );
}
