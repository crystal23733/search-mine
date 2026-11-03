use liar_protocol::consent::*;
#[test]
fn current_consent_accepts_closed_versioned_control_without_client_permission() {
    assert_eq!(
        decode_consent_read(br#"{"v":1}"#),
        Ok(ConsentReadInput { v: 1 })
    );
    assert_eq!(
        decode_consent_refresh(br#"{"v":1,"expected_revision":null}"#),
        Ok(ConsentRefreshInput {
            v: 1,
            expected_revision: None
        })
    );
    for (purpose, expected) in [
        ("analytics", ConsentWithdrawal::Analytics),
        ("advertising", ConsentWithdrawal::Advertising),
        ("all", ConsentWithdrawal::All),
    ] {
        assert_eq!(
            decode_consent_withdraw(format!(r#"{{"v":1,"purpose":"{purpose}"}}"#).as_bytes()),
            Ok(ConsentWithdrawInput {
                v: 1,
                purpose: expected
            })
        );
    }
    for value in [
        br#"{"v":1,"account_id":"mine"}"#.as_slice(),
        br#"{"v":1,"analytics":true}"#,
        br#"{"v":1,"tc_string":"permission"}"#,
        br#"{"v":1,"v":1}"#,
        b"{}",
    ] {
        assert_eq!(decode_consent_read(value), Err(ConsentApiError::Malformed));
    }
    assert_eq!(
        decode_consent_read(br#"{"v":2}"#),
        Err(ConsentApiError::UnsupportedVersion)
    );
    assert_eq!(
        decode_consent_refresh(br#"{"v":2}"#),
        Err(ConsentApiError::UnsupportedVersion)
    );
    assert_eq!(
        decode_consent_withdraw(br#"{"v":2,"purpose":"all"}"#),
        Err(ConsentApiError::UnsupportedVersion)
    );
    assert_eq!(
        decode_consent_read(&[b' '; MAX_CONSENT_BYTES + 1]),
        Err(ConsentApiError::Malformed)
    );
    assert_eq!(
        decode_consent_refresh(&[b' '; MAX_CONSENT_BYTES + 1]),
        Err(ConsentApiError::Malformed)
    );
    assert_eq!(
        decode_consent_withdraw(&[b' '; MAX_CONSENT_BYTES + 1]),
        Err(ConsentApiError::Malformed)
    );
}
#[test]
fn refresh_revision_is_only_a_canonical_non_nil_cas_condition() {
    let id = "a0000000-0000-4000-8000-000000000001";
    assert_eq!(
        decode_consent_refresh(format!(r#"{{"v":1,"expected_revision":"{id}"}}"#).as_bytes())
            .unwrap()
            .expected_revision
            .as_deref(),
        Some(id)
    );
    for revision in [
        "invalid",
        "00000000-0000-0000-0000-000000000000",
        "A0000000-0000-4000-8000-000000000001",
        "a0000000000040008000000000000001",
    ] {
        assert_eq!(
            decode_consent_refresh(
                format!(r#"{{"v":1,"expected_revision":"{revision}"}}"#).as_bytes()
            ),
            Err(ConsentApiError::Malformed)
        );
    }
    assert_eq!(
        decode_consent_refresh(br#"{"v":1,"expected_revision":null,"grant":true}"#),
        Err(ConsentApiError::Malformed)
    );
    assert_eq!(
        decode_consent_withdraw(br#"{"v":1,"purpose":"unknown"}"#),
        Err(ConsentApiError::Malformed)
    );
}
