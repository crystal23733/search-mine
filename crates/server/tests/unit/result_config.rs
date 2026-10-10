use super::*;
#[test]
fn defaults_and_strict_operator_bounds() {
    let c = load_result_config(|_| None).unwrap();
    assert_eq!((c.authorities, c.requests), (64, 16));
    for (key, max) in [
        ("LIAR_RESULT_AUTHORITIES", 20000),
        ("LIAR_RESULT_REQUESTS", 256),
    ] {
        for value in [
            "".into(),
            "0".into(),
            "-1".into(),
            " 2".into(),
            "+2".into(),
            "1.0".into(),
            (max + 1).to_string(),
            "9".repeat(50),
        ] {
            assert!(load_result_config(|name| (name == key).then(|| value.clone())).is_err());
        }
        assert!(load_result_config(|name| (name == key).then(|| max.to_string())).is_ok());
    }
}
