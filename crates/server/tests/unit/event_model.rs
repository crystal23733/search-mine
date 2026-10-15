use super::*;
use crate::events::{ConsentSnapshot, ConsentState, DedupDecision::*, admit_client, deduplicate};
use serde_json::{Value, json};

fn source(event: Value) -> Value {
    json!({"event_version":1,"event_id":"00000000-0000-0000-0000-000000000001","session_key":"00000000-0000-0000-0000-000000000002","occurred_at":1791331200,"locale":"ko","event":event})
}
fn decode(value: Value) -> Result<ClientDraft, EventError> {
    decode_client(&serde_json::to_vec(&value).unwrap())
}
fn allowed() -> ConsentSnapshot {
    let granted = ConsentState::Granted {
        revision: Uuid::from_u128(9),
        valid_from: 1791331200,
        expires_at: 1791331300,
    };
    ConsentSnapshot {
        subject: Some(EventId::parse("00000000-0000-0000-0000-000000000002").unwrap()),
        analytics: granted,
        advertising: granted,
    }
}
#[test]
fn all_closed_client_kinds_are_bounded_typed_claims_with_independent_purposes() {
    let id = "00000000-0000-0000-0000-000000000003";
    let actions = [
        json!({"type":"landing_view","route":"home"}),
        json!({"type":"invite_open"}),
        json!({"type":"tutorial_start","run_id":id}),
        json!({"type":"tutorial_complete","run_id":id,"duration_ms":3600000,"skipped":false}),
        json!({"type":"rematch_selected","previous_match_id":id}),
        json!({"type":"share_click","date":"2024-02-29","channel":"png"}),
        json!({"type":"ad_request","placement":"menu"}),
        json!({"type":"ad_impression","placement":"result"}),
        json!({"type":"ad_failed","placement":"menu","reason":"no_fill"}),
    ];
    for (index, action) in actions.into_iter().enumerate() {
        let event = admit_client(decode(source(action)).unwrap(), allowed(), 1791331200).unwrap();
        assert_eq!(
            event.purpose(),
            if index < 6 {
                Purpose::Analytics
            } else {
                Purpose::Advertising
            }
        );
        assert_eq!(
            event.subject,
            EventSubject::Session(EventId::parse("00000000-0000-0000-0000-000000000002").unwrap())
        );
        assert_eq!(event.id().uuid(), Uuid::from_u128(1));
        assert_eq!(event.occurred_at(), 1791331200);
    }
    for route in ["daily", "rules", "tutorial"] {
        assert!(decode(source(json!({"type":"landing_view","route":route}))).is_ok());
    }
    for channel in ["copy", "native"] {
        assert!(
            decode(source(
                json!({"type":"share_click","date":"2026-10-14","channel":channel})
            ))
            .is_ok()
        );
    }
    for reason in ["unavailable", "cancelled"] {
        assert!(
            decode(source(
                json!({"type":"ad_failed","placement":"result","reason":reason})
            ))
            .is_ok()
        );
    }
}
#[test]
fn a_client_cannot_assert_server_authority_consent_or_sensitive_fields() {
    let original = source(json!({"type":"invite_open"}));
    for key in [
        "source",
        "purpose",
        "account_id",
        "consent",
        "nickname",
        "email",
        "subject_digest",
        "ip",
        "token",
        "cookie",
        "seed",
        "payload",
    ] {
        let mut value = original.clone();
        value[key] = json!("forged");
        assert_eq!(decode(value), Err(EventError::Invalid), "{key}");
        let mut value = original.clone();
        value["event"][key] = json!("forged");
        assert_eq!(decode(value), Err(EventError::Invalid), "{key}");
    }
    for kind in [
        "match_end",
        "daily_verified",
        "consent_changed",
        "attack",
        "unknown",
    ] {
        assert_eq!(
            decode(source(json!({"type":kind}))),
            Err(EventError::Invalid)
        );
    }
    let mut value = original.clone();
    value["event"] = json!({"type":"landing_view","route":"/friends?code=SECRET"});
    assert!(decode(value).is_err());
    let mut value = original;
    value.as_object_mut().unwrap().remove("session_key");
    assert!(decode(value).is_err());
}
#[test]
fn byte_limits_canonical_identifiers_versions_locales_and_numeric_shapes_fail_closed() {
    let original = source(json!({"type":"invite_open"}));
    let mut bytes = serde_json::to_vec(&original).unwrap();
    bytes.resize(MAX_EVENT_BYTES, b' ');
    assert!(decode_client(&bytes).is_ok());
    bytes.push(b' ');
    assert_eq!(decode_client(&bytes), Err(EventError::Capacity));
    assert_eq!(decode_client(&[0xff]), Err(EventError::Invalid));
    for locale in ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"] {
        let mut v = original.clone();
        v["locale"] = json!(locale);
        assert!(decode(v).is_ok());
    }
    for locale in ["", "KO", "zh", "pt", "/en", "ko\n"] {
        let mut v = original.clone();
        v["locale"] = json!(locale);
        assert!(decode(v).is_err());
    }
    for key in ["event_id", "session_key"] {
        for id in [
            "00000000-0000-0000-0000-000000000000",
            "00000000000000000000000000000001",
            "00000000-0000-0000-0000-00000000000A",
            "not-id",
        ] {
            let mut v = original.clone();
            v[key] = json!(id);
            assert!(decode(v).is_err());
        }
    }
    let mut v = original.clone();
    v["event_version"] = json!(2);
    assert_eq!(decode(v), Err(EventError::Unsupported));
    for value in [
        json!(-1),
        json!(253402300800i64),
        json!(1.5),
        json!("1791331200"),
    ] {
        let mut v = original.clone();
        v["occurred_at"] = value;
        assert!(decode(v).is_err());
    }
    assert!(decode(source(json!({"type":"tutorial_complete","run_id":"00000000-0000-0000-0000-000000000003","duration_ms":3600001,"skipped":true}))).is_err());
    assert!(
        decode(source(
            json!({"type":"share_click","date":"2025-02-29","channel":"png"})
        ))
        .is_err()
    );
    assert!(decode_client(br#"{"event_version":1,"event_version":1}"#).is_err());
}
#[test]
fn retry_identity_includes_original_actor_time_purpose_and_content() {
    let value = source(json!({"type":"invite_open"}));
    let event = admit_client(decode(value.clone()).unwrap(), allowed(), 1791331200).unwrap();
    assert_eq!(deduplicate(&event, &event.clone()), Duplicate);
    for (key, value) in [
        ("session_key", json!("00000000-0000-0000-0000-000000000003")),
        ("occurred_at", json!(1791331201)),
        ("locale", json!("en")),
        ("event", json!({"type":"ad_request","placement":"menu"})),
    ] {
        let mut changed = source(json!({"type":"invite_open"}));
        changed[key] = value;
        let mut consent = allowed();
        consent.subject = Some(EventId::parse(changed["session_key"].as_str().unwrap()).unwrap());
        let next = admit_client(decode(changed).unwrap(), consent, 1791331201).unwrap();
        assert_eq!(deduplicate(&event, &next), Conflict);
    }
    let mut changed = value;
    changed["event_id"] = json!("00000000-0000-0000-0000-000000000004");
    let next = admit_client(decode(changed).unwrap(), allowed(), 1791331200).unwrap();
    assert_eq!(deduplicate(&event, &next), New);
}
#[test]
fn essential_facts_are_a_separate_minimal_rust_constructor_without_hidden_game_data() {
    let id = EventId::parse("00000000-0000-0000-0000-000000000001").unwrap();
    for fact in [
        ServerFact::MatchEnd {
            match_id: id,
            opponent: OpponentKind::Bot,
            reason: PublicEndReason::Timeout,
        },
        ServerFact::DailyVerified {
            completion_id: id,
            date: "2026-10-14".into(),
            mistakes: 2,
            trust: DailyTrust::VerifiedCompletion,
        },
    ] {
        let event = ProductEvent::server(id, id, 1791331200, fact.clone()).unwrap();
        assert_eq!(event.purpose(), Purpose::Essential);
        let text = serde_json::to_string(&event).unwrap();
        for excluded in [
            "nickname",
            "email",
            "seed",
            "token",
            "cookie",
            "cell",
            "replay",
            "session_key",
        ] {
            assert!(!text.contains(excluded));
        }
        assert_eq!(
            ProductEvent::server(id, id, -1, fact),
            Err(EventError::Invalid)
        );
    }
    assert!(
        ProductEvent::server(
            id,
            id,
            1,
            ServerFact::DailyVerified {
                completion_id: id,
                date: "invalid".into(),
                mistakes: 0,
                trust: DailyTrust::VerifiedTime
            }
        )
        .is_err()
    );
}
