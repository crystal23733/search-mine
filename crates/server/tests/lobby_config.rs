use liar_server::lobby::*;
#[test]
fn bounded_runtime_defaults_and_valid_overrides_are_accepted() {
    let config = load_lobby_config(|_| None).unwrap();
    assert_eq!(
        (
            config.limits.capacity,
            config.limits.workers,
            config.authorities,
            config.requests,
            config.board_capacity,
            config.board_workers,
            config.bot_workers
        ),
        (32, 2, 64, 16, 4, 1, 2)
    );
    let config = load_lobby_config(|key| match key {
        "LIAR_LOBBY_CAPACITY" => Some("8".into()),
        "LIAR_BOARD_CAPACITY" => Some("2".into()),
        _ => None,
    })
    .unwrap();
    assert_eq!((config.limits.capacity, config.board_capacity), (8, 2));
}
#[test]
fn malformed_or_excessive_limits_and_workers_above_capacity_fail_closed() {
    for (name, value) in [
        ("LIAR_LOBBY_CAPACITY", "0"),
        ("LIAR_LOBBY_CAPACITY", "4097"),
        ("LIAR_LOBBY_REQUESTS", "257"),
        ("LIAR_BOARD_CAPACITY", "257"),
        ("LIAR_BOARD_WORKERS", "65"),
        ("LIAR_BOT_WORKERS", "0"),
        ("LIAR_LOBBY_AUTHORITIES", "0"),
        ("LIAR_LOBBY_AUTHORITIES", "20001"),
        ("LIAR_LOBBY_CAPACITY", "1"),
        ("LIAR_LOBBY_WORKERS", "65"),
        ("LIAR_LOBBY_WORKERS", ""),
        ("LIAR_LOBBY_CAPACITY", " 32"),
        ("LIAR_LOBBY_REQUESTS", "1e2"),
        ("LIAR_LOBBY_REQUESTS", "-1"),
    ] {
        assert!(
            load_lobby_config(|key| (key == name).then(|| value.into())).is_err(),
            "{name}={value}"
        );
    }
    assert!(
        load_lobby_config(|key| match key {
            "LIAR_BOARD_CAPACITY" => Some("1".into()),
            "LIAR_BOARD_WORKERS" => Some("2".into()),
            _ => None,
        })
        .is_err()
    );
}
