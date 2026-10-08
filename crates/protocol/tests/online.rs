use liar_protocol::{game::PublicError, online::decode_online};
use serde_json::json;

fn input() -> serde_json::Value {
    json!({"v":1,"match_id":"00000000-0000-4000-8000-000000000001","command_id":"00000000-0000-4000-8000-000000000002","client_seq":1,"session_epoch":2,"known_revision":0,"action":{"type":"open","cell":18}})
}

#[test]
fn canonical_online_input_has_only_intent_and_server_assigned_epoch() {
    let command = decode_online(&input().to_string()).unwrap();
    assert_eq!(command.client_seq, 1);
    assert_eq!(command.session_epoch, 2);
    assert_eq!(serde_json::to_value(command).unwrap(), input());
}

#[test]
fn rejects_forged_clocks_seats_targets_and_malformed_uuid_or_numeric_fields() {
    for key in ["seed", "seat", "received_at", "csrf", "target"] {
        let mut value = input();
        value[key] = json!(1);
        assert_eq!(
            decode_online(&value.to_string()),
            Err(PublicError::Malformed)
        );
    }
    for key in ["match_id", "command_id"] {
        for invalid in [
            "00000000-0000-0000-0000-000000000000",
            "00000000000040008000000000000001",
            "ABCDEF00-0000-4000-8000-000000000001",
            "x",
        ] {
            let mut value = input();
            value[key] = json!(invalid);
            assert_eq!(
                decode_online(&value.to_string()),
                Err(PublicError::Malformed)
            );
        }
    }
    for key in ["client_seq", "session_epoch"] {
        let mut value = input();
        value[key] = json!(0);
        assert_eq!(
            decode_online(&value.to_string()),
            Err(PublicError::Malformed)
        );
    }
    for cell in [json!(-1), json!(256), json!(1.2)] {
        let mut value = input();
        value["action"]["cell"] = cell;
        assert_eq!(
            decode_online(&value.to_string()),
            Err(PublicError::InvalidCell)
        );
    }
    let mut value = input();
    value["action"]["target"] = json!(1);
    assert_eq!(
        decode_online(&value.to_string()),
        Err(PublicError::Malformed)
    );
}

#[test]
fn rejects_duplicate_fields_oversized_frames_and_reports_unsupported_version() {
    let value = input().to_string();
    let duplicate = value.replacen("\"v\":1", "\"v\":1,\"v\":1", 1);
    assert_eq!(decode_online(&duplicate), Err(PublicError::Malformed));
    assert_eq!(
        decode_online(&"x".repeat(8193)),
        Err(PublicError::Malformed)
    );
    let mut value = input();
    value["v"] = json!(2);
    assert_eq!(
        decode_online(&value.to_string()),
        Err(PublicError::UnsupportedVersion)
    );
}
