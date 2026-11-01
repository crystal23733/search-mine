use liar_protocol::me::*;
#[test]
fn self_views_accept_only_their_closed_bounded_contract() {
    assert_eq!(
        decode_me_overview(br#"{"v":1}"#),
        Ok(MeOverviewInput { v: 1 })
    );
    let source = br#"{"v":1,"kind":"online","opponent":"all","cursor":null}"#;
    assert!(decode_me_history(source).is_ok());
    assert!(decode_me_history(br#"{"v":1,"kind":"daily","opponent":"all"}"#).is_ok());
    for source in [
        br#"{"v":1,"account_id":"x"}"#.as_slice(),
        br#"{"v":1,"v":1}"#,
        br#"{"v":1.0}"#,
        b"{}",
    ] {
        assert_eq!(decode_me_overview(source), Err(MeError::Malformed));
    }
    for source in [
        r#"{"v":1,"kind":"daily","opponent":"human"}"#,
        r#"{"v":1,"kind":"online","opponent":"all","limit":999}"#,
        r#"{"v":1,"kind":"raw","opponent":"all"}"#,
        r#"{"v":1,"kind":"online","opponent":"unknown"}"#,
    ] {
        assert_eq!(
            decode_me_history(source.as_bytes()),
            Err(MeError::Malformed)
        );
    }
    assert_eq!(
        decode_me_overview(br#"{"v":2}"#),
        Err(MeError::UnsupportedVersion)
    );
    assert_eq!(
        decode_me_history(br#"{"v":2,"kind":"online","opponent":"all"}"#),
        Err(MeError::UnsupportedVersion)
    );
    assert_eq!(
        decode_me_overview(&[b' '; MAX_ME_BYTES + 1]),
        Err(MeError::Malformed)
    );
    assert_eq!(
        decode_me_history(&[b' '; MAX_ME_BYTES + 1]),
        Err(MeError::Malformed)
    );
    assert!(
        decode_me_history(
            format!(
                r#"{{"v":1,"kind":"online","opponent":"all","cursor":"{}"}}"#,
                "a".repeat(513)
            )
            .as_bytes()
        )
        .is_err()
    );
}
