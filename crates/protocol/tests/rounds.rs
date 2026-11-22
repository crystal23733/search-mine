use liar_protocol::{game::PublicError, rounds::decode_round};
use serde_json::json;

#[test]
fn round_commands_only_carry_a_cursor_and_an_existing_game_action() {
    let request = json!({
        "v": 2,
        "round_id": "a0000000-0000-4000-8000-000000000001",
        "command_id": "a0000000-0000-4000-8000-000000000002",
        "client_seq": 1,
        "session_epoch": 1,
        "known_revision": 0,
        "action": { "type": "attack" }
    });
    assert!(decode_round(&request.to_string()).is_ok());
    for (field, value, error) in [
        (
            "action",
            json!({"type":"attack", "target":3}),
            PublicError::Malformed,
        ),
        (
            "action",
            json!({"type":"open", "cell":256}),
            PublicError::InvalidCell,
        ),
        ("sender_seat", json!(3), PublicError::Malformed),
        ("received_at", json!(123), PublicError::Malformed),
        ("session_epoch", json!(0), PublicError::Malformed),
        (
            "known_revision",
            json!(9007199254740992_u64),
            PublicError::Malformed,
        ),
        ("v", json!(1), PublicError::UnsupportedVersion),
    ] {
        let mut changed = request.clone();
        changed[field] = value;
        assert_eq!(decode_round(&changed.to_string()), Err(error));
    }
}
