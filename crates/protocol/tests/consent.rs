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
            expected_revision: None,
            evidence: None,
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
#[test]
fn evidence_is_bounded_closed_and_redacted_without_accepting_client_purpose_flags() {
    let encode = |evidence: serde_json::Value| {
        serde_json::to_vec(&serde_json::json!({"v":1,"expected_revision":null,"evidence":evidence}))
            .unwrap()
    };
    let evidence = serde_json::json!({"provider":"google_tcf_v2","tc_string":"AB-c.DE_f"});
    let value = decode_consent_refresh(&encode(evidence.clone())).unwrap();
    assert_eq!(value.evidence.as_ref().unwrap().tc_string, "AB-c.DE_f");
    assert!(!format!("{value:?}").contains("AB-c.DE_f"));
    for raw in ["", ".ABC", "ABC.", "A..B", "A.B.C.D.E", "ABC=", "é"] {
        assert!(
            decode_consent_refresh(&encode(
                serde_json::json!({"provider":"google_tcf_v2","tc_string":raw})
            ))
            .is_err()
        );
    }
    assert!(
        decode_consent_refresh(&encode(
            serde_json::json!({"provider":"google_tcf_v2","tc_string":"A".repeat(MAX_TCF_CHARS+1)})
        ))
        .is_err()
    );
    for key in ["analytics", "advertising", "account_id"] {
        let mut invalid = evidence.clone();
        invalid[key] = serde_json::json!(true);
        assert!(decode_consent_refresh(&encode(invalid)).is_err());
    }
    assert!(
        decode_consent_refresh(&encode(
            serde_json::json!({"provider":"other","tc_string":"ABC"})
        ))
        .is_err()
    );
}
