use liar_core::rules::{RulesError, RulesSnapshot};
const SOURCE: &str = include_str!("../../../config/game-rules.toml");
#[test]
fn one_toml_source_loads_public_rules_and_semantic_hash() {
    let snapshot = RulesSnapshot::load(SOURCE).unwrap();
    assert_eq!(
        snapshot.rules.board_spec(),
        liar_core::board::BoardSpec::default()
    );
    assert_eq!(snapshot.rules.gauge_capacity, 20);
    assert_eq!(snapshot.rules.max_lies, 2);
    assert_eq!(snapshot.hash.len(), 64);
    assert_eq!(
        snapshot,
        RulesSnapshot::load(&format!("# whitespace\n{SOURCE}\n")).unwrap()
    );
    let changed = RulesSnapshot::load(&SOURCE.replace("240000", "240001")).unwrap();
    assert_ne!(snapshot.hash, changed.hash);
}
#[test]
fn malformed_unknown_or_out_of_range_config_is_rejected() {
    for source in [
        SOURCE.replace("width = 16", "width = 0"),
        SOURCE.replace("mines = 40", "mines = 256"),
        SOURCE.replace("max_lies = 2", "max_lies = 3"),
        SOURCE.replace("safe_gain = 1", "safe_gain = 21"),
        SOURCE.replace("duration_ms = 240000", "duration_ms = -1"),
        format!("{SOURCE}\npassword = 'never'"),
        "[invalid".into(),
    ] {
        assert!(RulesSnapshot::load(&source).is_err());
    }
    assert_eq!(
        RulesSnapshot::load(&"x".repeat(16385)),
        Err(RulesError::Malformed)
    );
}
#[test]
fn snapshot_hash_versions_strategy_and_all_operational_ranges_are_verified() {
    let snapshot = RulesSnapshot::bundled();
    snapshot.verify().unwrap();
    let mut altered = snapshot.clone();
    altered.hash = "0".repeat(64);
    assert_eq!(altered.verify(), Err(RulesError::HashMismatch));
    altered = snapshot.clone();
    altered.solver_version += 1;
    assert_eq!(altered.verify(), Err(RulesError::HashMismatch));
    for (key, value) in [
        ("version = 1", "version = 2"),
        ("mines = 40", "mines = 0"),
        ("opening = 0", "opening = 999"),
        ("duration_ms = 240000", "duration_ms = 0"),
        ("countdown_ms = 3000", "countdown_ms = 10001"),
        ("mine_stun_ms = 3000", "mine_stun_ms = 0"),
        ("gauge_capacity = 20", "gauge_capacity = 0"),
        ("safe_gain = 1", "safe_gain = 0"),
        ("reconnect_grace_ms = 30000", "reconnect_grace_ms = 0"),
        ("reconnect_grace_ms = 30000", "reconnect_grace_ms = 300001"),
        ("room_expiry_ms = 600000", "room_expiry_ms = 0"),
        ("max_commands_per_seat = 8192", "max_commands_per_seat = 0"),
        ("known-neighborhood-v1", "unknown-strategy"),
    ] {
        assert!(
            RulesSnapshot::load(&SOURCE.replace(key, value)).is_err(),
            "{key}"
        );
    }
}
