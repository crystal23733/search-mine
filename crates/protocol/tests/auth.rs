use liar_protocol::auth::{AuthAccount, AuthExport, AuthIdentity};

#[test]
fn login_request_has_one_rust_contract_with_an_optional_invitation_and_closed_fields() {
    use liar_protocol::auth::AuthLoginRequest;
    let plain: AuthLoginRequest =
        serde_json::from_str(r#"{"locale":"en","return_path":"home"}"#).unwrap();
    assert!(plain.invite_code.is_none());
    assert!(
        serde_json::to_value(plain)
            .unwrap()
            .get("invite_code")
            .is_none()
    );
    let invited: AuthLoginRequest =
        serde_json::from_str(r#"{"locale":"fr","return_path":"friends","invite_code":"ABCD2345"}"#)
            .unwrap();
    assert_eq!(invited.invite_code.as_deref(), Some("ABCD2345"));
    for raw in [
        r#"{"locale":"en","return_path":"friends","invite_code":"ABCD2345","invite_code":"ZZZZ6789"}"#,
        r#"{"locale":"en","return_path":"home","account_id":"forged"}"#,
    ] {
        assert!(serde_json::from_str::<AuthLoginRequest>(raw).is_err());
    }
    assert!(liar_protocol::public_types().contains("invite_code?: string"));
}

#[test]
fn auth_epoch_seconds_agree_between_json_and_generated_types() {
    let value = AuthExport {
        account: AuthAccount {
            id: "fixture-account".into(),
            nickname: None,
        },
        created_at: 1,
        last_seen_at: 253_402_300_799,
        identities: vec![AuthIdentity {
            provider: "google".into(),
            linked_at: 2,
        }],
    };
    let json = serde_json::to_value(value).unwrap();
    assert_eq!(json["last_seen_at"], serde_json::json!(253_402_300_799_i64));
    assert!(json["identities"][0]["linked_at"].is_number());
    let types = liar_protocol::public_types();
    for field in ["linked_at", "created_at", "last_seen_at"] {
        assert!(
            types.contains(&format!("{field}: number")),
            "JSON epoch seconds need number declarations: {field}"
        );
    }
}
