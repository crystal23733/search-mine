use liar_server::auth::{AeadVault, CredentialPurpose, CredentialVault, DigestKeys, Provider};
use liar_server::auth::{AuthError, Nickname, SecretToken};
use uuid::Uuid;

#[test]
fn provider_and_return_boundaries_reject_arbitrary_identity_or_redirect_targets() {
    for provider in [
        Provider::Google,
        Provider::Apple,
        Provider::Kakao,
        Provider::Naver,
    ] {
        assert_eq!(Provider::parse(provider.as_str()).unwrap(), provider);
        assert!(provider.issuer().starts_with("https://"));
    }
    for value in ["discord", "GOOGLE", "google/../apple", ""] {
        assert!(Provider::parse(value).is_err())
    }
    for path in [
        liar_server::auth::ReturnPath::Home,
        liar_server::auth::ReturnPath::Daily,
        liar_server::auth::ReturnPath::Friends,
        liar_server::auth::ReturnPath::Settings,
    ] {
        assert_eq!(
            liar_server::auth::ReturnPath::parse(path.as_str()).unwrap(),
            path
        );
    }
    for value in [
        "https://evil.example/",
        "//evil.example/",
        "/home",
        "home?next=https://evil.example",
    ] {
        assert!(liar_server::auth::ReturnPath::parse(value).is_err())
    }
    use liar_server::auth::AuthIntent;
    let account = Uuid::new_v4();
    for intent in [
        AuthIntent::Login,
        AuthIntent::Link(account),
        AuthIntent::Reauth(account),
    ] {
        assert_eq!(
            AuthIntent::restore(intent.kind(), intent.account()).unwrap(),
            intent
        );
    }
    for (kind, account) in [
        ("login", Some(account)),
        ("link", None),
        ("reauth", None),
        ("delete", None),
    ] {
        assert!(AuthIntent::restore(kind, account).is_err())
    }
}

#[test]
fn session_reauthentication_is_bounded_and_pkce_matches_the_rfc_vector() {
    use liar_server::auth::{Account, Session};
    let session = Session {
        id: Uuid::new_v4(),
        account: Account {
            id: Uuid::new_v4(),
            nickname: None,
        },
        created_at: 1000,
        expires_at: 2000,
        authenticated_at: 1010,
    };
    assert!(!session.fresh(999, 300));
    assert!(!session.fresh(1009, 300));
    assert!(session.fresh(1010, 300));
    assert!(session.fresh(1309, 300));
    assert!(!session.fresh(1310, 300));
    assert!(!session.fresh(2000, 3000));
    assert!(!session.fresh(1010, 0));
    assert_eq!(
        SecretToken::parse("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk")
            .unwrap()
            .pkce_challenge(),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
}

#[test]
fn subject_digest_is_namespaced_and_rotates_without_losing_old_lookup() {
    let old = DigestKeys::new(1, vec![(1, [1; 32])]).unwrap();
    let rotated = DigestKeys::new(2, vec![(1, [1; 32]), (2, [2; 32])]).unwrap();
    let before = old.digest(Provider::Google, "subject").unwrap();
    let after = rotated.digest(Provider::Google, "subject").unwrap();
    assert_eq!(after.len(), 2);
    assert_eq!(after[0].0, 2);
    assert_eq!(after[1], before[0]);
    assert_ne!(after[0].1, before[0].1);
    assert_ne!(after, rotated.digest(Provider::Apple, "subject").unwrap());
    assert_ne!(after, rotated.digest(Provider::Google, "subject2").unwrap());
    assert!(rotated.digest(Provider::Google, "").is_err());
    assert!(rotated.digest(Provider::Google, &"x".repeat(513)).is_err());
    assert!(DigestKeys::new(0, vec![]).is_err());
    assert!(DigestKeys::new(1, vec![(2, [1; 32])]).is_err());
    assert!(DigestKeys::new(1, vec![(1, [1; 32]), (1, [2; 32])]).is_err());
}

#[test]
fn aead_secrets_are_randomized_and_reject_tamper_other_identity_or_purpose() {
    let vault = AeadVault::new(1, vec![(1, [1; 32])]).unwrap();
    let id = Uuid::new_v4();
    let secret = b"not-a-real-refresh-credential";
    let first = vault
        .seal(id, Provider::Apple, CredentialPurpose::AppleRevoke, secret)
        .unwrap();
    let second = vault
        .seal(id, Provider::Apple, CredentialPurpose::AppleRevoke, secret)
        .unwrap();
    assert_ne!(first, second);
    assert!(!first.windows(secret.len()).any(|w| w == secret));
    assert_eq!(
        vault
            .open(id, Provider::Apple, CredentialPurpose::AppleRevoke, &first)
            .unwrap()
            .as_slice(),
        secret
    );
    assert!(
        vault
            .open(
                Uuid::new_v4(),
                Provider::Apple,
                CredentialPurpose::AppleRevoke,
                &first
            )
            .is_err()
    );
    assert!(
        vault
            .open(id, Provider::Google, CredentialPurpose::Pkce, &first)
            .is_err()
    );
    assert!(
        vault
            .open(id, Provider::Apple, CredentialPurpose::Pkce, &first)
            .is_err()
    );
    let mut tampered = first.clone();
    *tampered.last_mut().unwrap() ^= 1;
    assert!(
        vault
            .open(
                id,
                Provider::Apple,
                CredentialPurpose::AppleRevoke,
                &tampered
            )
            .is_err()
    );
    assert!(
        vault
            .open(
                id,
                Provider::Apple,
                CredentialPurpose::AppleRevoke,
                &first[..12]
            )
            .is_err()
    );
    let rotated = AeadVault::new(2, vec![(1, [1; 32]), (2, [2; 32])]).unwrap();
    assert!(
        rotated
            .open(id, Provider::Apple, CredentialPurpose::AppleRevoke, &first)
            .is_ok()
    );
    assert!(
        AeadVault::new(1, vec![(1, [2; 32])])
            .unwrap()
            .open(id, Provider::Apple, CredentialPurpose::AppleRevoke, &first)
            .is_err()
    );
    assert!(
        vault
            .seal(id, Provider::Google, CredentialPurpose::AppleRevoke, secret)
            .is_err()
    );
    assert!(
        vault
            .seal(id, Provider::Apple, CredentialPurpose::AppleRevoke, &[])
            .is_err()
    );
}

#[test]
fn nickname_normalizes_unicode_and_rejects_unsafe_or_unbounded_input() {
    assert_eq!(Nickname::parse("e\u{301}가").unwrap().as_str(), "é가");
    for name in ["한국 이름", "日本語", "AB12", "äöü", "مرحبا"] {
        assert_eq!(Nickname::parse(name).unwrap().as_str(), name);
    }
    for name in [
        "",
        "a",
        " abc",
        "abc ",
        "<script>",
        "a\u{202e}b",
        "a\nb",
        "😀😀",
        "a\u{200d}b",
        "a\u{034f}b",
        "\u{0301}a",
    ] {
        assert_eq!(Nickname::parse(name), Err(AuthError::Invalid), "{name:?}");
    }
    assert!(Nickname::parse(&"가".repeat(16)).is_ok());
    assert!(Nickname::parse(&"가".repeat(17)).is_err());
    assert!(Nickname::parse(&format!("a{}b", "\u{0301}".repeat(300))).is_err());
}

#[test]
fn opaque_tokens_are_random_canonical_and_redacted() {
    let first = SecretToken::generate().unwrap();
    let second = SecretToken::generate().unwrap();
    assert_ne!(first.hash(), second.hash());
    let value = first.expose();
    assert_eq!(value.len(), 43);
    assert_eq!(SecretToken::parse(&value).unwrap().hash(), first.hash());
    assert_ne!(first.hash(), [0; 32]);
    assert!(!format!("{first:?}").contains(value.as_str()));
    for value in [
        "",
        "abc",
        &"a".repeat(44),
        &format!("{}=", first.expose().as_str()),
        &"!".repeat(43),
    ] {
        assert!(SecretToken::parse(value).is_err());
    }
}
