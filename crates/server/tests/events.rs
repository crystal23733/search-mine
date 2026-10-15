use liar_server::events::decode_client;

#[test]
fn a_closed_valid_optional_event_has_a_normalized_draft() {
    let source = br#"{"event_version":1,"event_id":"00000000-0000-0000-0000-000000000001","session_key":"00000000-0000-0000-0000-000000000002","occurred_at":1791331200,"locale":"ko","event":{"type":"landing_view","route":"home"}}"#;
    assert!(
        decode_client(source).is_ok(),
        "A valid optional draft needs a typed event contract"
    );
}
