use liar_server::auth::*;
use std::collections::HashMap;
use zeroize::Zeroizing;

fn configured() -> HashMap<String, String> {
    let key = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE";
    [ ("LIAR_PUBLIC_ORIGIN","https://game.example"),("LIAR_AUTH_CSRF_KEY",key),
      ("LIAR_AUTH_DIGEST_KEYS",r#"{"current":1,"keys":[{"version":1,"key":"AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI"}]}"#),
      ("LIAR_AUTH_VAULT_KEYS",r#"{"current":1,"keys":[{"version":1,"key":"AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM"}]}"#),
      ("LIAR_GOOGLE_CLIENT_ID","google-client"),("LIAR_GOOGLE_CLIENT_SECRET","google-secret"),
      ("LIAR_KAKAO_CLIENT_ID","kakao-client"),("LIAR_KAKAO_CLIENT_SECRET","kakao-secret"),
      ("LIAR_NAVER_CLIENT_ID","naver-client"),("LIAR_NAVER_CLIENT_SECRET","naver-secret"),
      ("LIAR_APPLE_CLIENT_ID","apple-services-id"),("LIAR_APPLE_TEAM_ID","TEAMTEST01"),
      ("LIAR_APPLE_KEY_ID","KEYTEST001"),("LIAR_APPLE_PRIVATE_KEY_FILE","fixture")]
      .into_iter().map(|(k,v)|(k.into(),v.into())).collect()
}
#[test]
fn configuration_disables_absent_auth_and_rejects_partial_or_invalid_secrets() {
    assert!(
        load_auth_config(|_| None, |_| unreachable!())
            .unwrap()
            .is_none()
    );
    let read_key = |_: &str| {
        Ok(Zeroizing::new(
            include_bytes!("fixtures/auth/apple-test-only.pem").to_vec(),
        ))
    };
    let values = configured();
    let config = load_auth_config(|k| values.get(k).cloned(), read_key)
        .unwrap()
        .expect("Explicit configuration must enable four providers");
    assert_eq!(config.providers.len(), 4);
    assert_eq!(config.security.origin(), "https://game.example");
    for missing in [
        "LIAR_AUTH_CSRF_KEY",
        "LIAR_AUTH_DIGEST_KEYS",
        "LIAR_AUTH_VAULT_KEYS",
        "LIAR_GOOGLE_CLIENT_SECRET",
        "LIAR_APPLE_KEY_ID",
    ] {
        let mut partial = values.clone();
        partial.remove(missing);
        assert!(
            load_auth_config(|k| partial.get(k).cloned(), read_key).is_err(),
            "{missing}"
        );
    }
    for (key, value) in [
        ("LIAR_PUBLIC_ORIGIN", "http://localhost"),
        ("LIAR_PUBLIC_ORIGIN", "https://game.example/path"),
        ("LIAR_AUTH_CSRF_KEY", "bad"),
        ("LIAR_APPLE_KEY_ID", "bad"),
        ("LIAR_AUTH_DIGEST_KEYS", r#"{"current":2,"keys":[]}"#),
    ] {
        let mut invalid = values.clone();
        invalid.insert(key.into(), value.into());
        assert!(
            load_auth_config(|k| invalid.get(k).cloned(), read_key).is_err(),
            "{key}"
        );
    }
    let no_providers: HashMap<_, _> = values
        .iter()
        .filter(|(k, _)| k.starts_with("LIAR_AUTH_") || *k == "LIAR_PUBLIC_ORIGIN")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    assert!(
        load_auth_config(|k| no_providers.get(k).cloned(), |_| unreachable!())
            .unwrap()
            .unwrap()
            .providers
            .is_empty()
    );
}
#[test]
fn apple_notification_audience_is_explicit_and_invalid_configuration_fails_closed() {
    let values = configured();
    let read_key = |_: &str| {
        Ok(Zeroizing::new(
            include_bytes!("fixtures/auth/apple-test-only.pem").to_vec(),
        ))
    };
    for audience in ["", "bad\naudience"] {
        let mut v = values.clone();
        v.insert("LIAR_APPLE_NOTIFICATION_AUDIENCE".into(), audience.into());
        assert!(load_auth_config(|k| v.get(k).cloned(), read_key).is_err());
    }
}
