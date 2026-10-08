use liar_protocol::auth::{AuthAccount, AuthExport, AuthIdentity};

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
