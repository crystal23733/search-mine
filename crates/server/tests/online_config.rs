use liar_server::online::*;
#[test]
fn malformed_or_unbounded_runtime_limits_fail_closed() {
    let default = load_online_config(|_| None).unwrap();
    assert_eq!(default.connections, 32);
    for (name, value) in [
        ("LIAR_ONLINE_MATCHES", "0"),
        ("LIAR_ONLINE_MATCHES", "10001"),
        ("LIAR_ONLINE_MAILBOX", "3"),
        ("LIAR_ONLINE_OUTGOING", "257"),
        ("LIAR_ONLINE_PROOF_WORKERS", "65"),
        ("LIAR_ONLINE_CONNECTIONS", "20001"),
        ("LIAR_ONLINE_CONNECTIONS", "-1"),
        ("LIAR_ONLINE_MATCHES", ""),
        ("LIAR_ONLINE_MAILBOX", " 64"),
        ("LIAR_ONLINE_CONNECTIONS", "1e3"),
    ] {
        assert!(
            load_online_config(|key| (key == name).then(|| value.to_string())).is_err(),
            "{name}={value}"
        );
    }
    let explicit = load_online_config(|key| match key {
        "LIAR_ONLINE_MATCHES" => Some("3".into()),
        "LIAR_ONLINE_CONNECTIONS" => Some("6".into()),
        _ => None,
    })
    .unwrap();
    assert_eq!(explicit.limits.matches, 3);
    assert_eq!(explicit.connections, 6);
}
