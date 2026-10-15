use super::*;
use crate::events::decode_client;
use serde_json::json;
fn draft(ad: bool) -> ClientDraft {
    decode_client(&serde_json::to_vec(&json!({"event_version":1,"event_id":"00000000-0000-0000-0000-000000000001","session_key":"00000000-0000-0000-0000-000000000002","occurred_at":1000,"locale":"en","event": if ad {json!({"type":"ad_impression","placement":"result"})} else {json!({"type":"invite_open"})}})).unwrap()).unwrap()
}
fn grant() -> ConsentState {
    ConsentState::Granted {
        revision: Uuid::from_u128(1),
        valid_from: 1000,
        expires_at: 2000,
    }
}
#[test]
fn optional_purposes_require_their_own_live_authoritative_grant_at_event_and_collection() {
    for ad in [false, true] {
        for denied in [
            ConsentState::Unknown,
            ConsentState::Denied,
            ConsentState::Failed,
        ] {
            let snapshot = if ad {
                ConsentSnapshot {
                    subject: Some(EventId::parse("00000000-0000-0000-0000-000000000002").unwrap()),
                    analytics: grant(),
                    advertising: denied,
                }
            } else {
                ConsentSnapshot {
                    subject: Some(EventId::parse("00000000-0000-0000-0000-000000000002").unwrap()),
                    analytics: denied,
                    advertising: grant(),
                }
            };
            assert_eq!(
                admit_client(draft(ad), snapshot, 1000),
                Err(EventError::Consent)
            );
        }
        let snapshot = ConsentSnapshot {
            subject: Some(EventId::parse("00000000-0000-0000-0000-000000000002").unwrap()),
            analytics: grant(),
            advertising: grant(),
        };
        assert!(admit_client(draft(ad), snapshot, 1000).is_ok());
        assert!(admit_client(draft(ad), snapshot, 1999).is_ok());
        assert_eq!(
            admit_client(draft(ad), snapshot, 2000),
            Err(EventError::Consent)
        );
        assert_eq!(
            admit_client(draft(ad), snapshot, 999),
            Err(EventError::Invalid)
        );
    }
    assert_eq!(
        admit_client(draft(false), ConsentSnapshot::default(), 1000),
        Err(EventError::Consent)
    );
    for (revision, valid_from, expires_at) in [
        (Uuid::nil(), 1000, 2000),
        (Uuid::from_u128(1), 1001, 2000),
        (Uuid::from_u128(1), 2000, 1000),
    ] {
        assert_eq!(
            admit_client(
                draft(false),
                ConsentSnapshot {
                    subject: Some(EventId::parse("00000000-0000-0000-0000-000000000002").unwrap()),
                    analytics: ConsentState::Granted {
                        revision,
                        valid_from,
                        expires_at
                    },
                    advertising: grant()
                },
                1100
            ),
            Err(EventError::Consent)
        );
    }
    for (valid_from, expires_at) in [(-1, 2000), (0, MAX_EVENT_TIMESTAMP + 1)] {
        assert_eq!(
            admit_client(
                draft(false),
                ConsentSnapshot {
                    subject: Some(EventId::parse("00000000-0000-0000-0000-000000000002").unwrap()),
                    analytics: ConsentState::Granted {
                        revision: Uuid::from_u128(1),
                        valid_from,
                        expires_at
                    },
                    advertising: grant()
                },
                1100
            ),
            Err(EventError::Invalid)
        );
    }
}
#[test]
fn another_or_missing_session_cannot_reuse_a_live_purpose_grant() {
    for subject in [
        None,
        Some(EventId::parse("00000000-0000-0000-0000-000000000003").unwrap()),
    ] {
        assert_eq!(
            admit_client(
                draft(false),
                ConsentSnapshot {
                    subject,
                    analytics: grant(),
                    advertising: grant()
                },
                1000
            ),
            Err(EventError::Consent)
        );
    }
}
#[test]
fn raw_exact_thirty_days_is_not_extended_by_retry_or_dst() {
    let at = UtcDay::parse("2026-03-08").unwrap().start();
    assert_eq!(raw_event_alive(at, at + RAW_EVENT_SECONDS - 1), Ok(()));
    assert_eq!(
        raw_event_alive(at, at + RAW_EVENT_SECONDS),
        Err(EventError::Expired)
    );
    assert_eq!(raw_event_alive(at, at - 1), Err(EventError::Invalid));
    for (first, last) in [
        (-1, 1),
        (0, -1),
        (MAX_EVENT_TIMESTAMP + 1, MAX_EVENT_TIMESTAMP + 1),
        (0, i64::MAX),
    ] {
        assert_eq!(raw_event_alive(first, last), Err(EventError::Invalid));
    }
    assert!(raw_event_alive(MAX_EVENT_TIMESTAMP, MAX_EVENT_TIMESTAMP).is_ok());
    assert_eq!(
        admit_client(
            draft(false),
            ConsentSnapshot::default(),
            1000 + RAW_EVENT_SECONDS
        ),
        Err(EventError::Expired)
    );
}
#[test]
fn calendar_thirteen_months_has_exact_utc_expiry_including_month_end_and_leap_year() {
    for (start, end) in [
        ("2024-01-31", "2025-02-28"),
        ("2023-01-31", "2024-02-29"),
        ("2024-02-29", "2025-03-29"),
        ("2026-03-08", "2027-04-08"),
    ] {
        let day = UtcDay::parse(start).unwrap();
        let expiry = UtcDay::parse(end).unwrap().start();
        assert_eq!(aggregate_expires_at(day), Ok(expiry));
        assert_eq!(aggregate_alive(day, expiry - 1), Ok(()));
        assert_eq!(aggregate_alive(day, expiry), Err(EventError::Expired));
        assert_eq!(
            aggregate_alive(day, day.start() - 1),
            Err(EventError::Invalid)
        );
    }
    assert_eq!(
        aggregate_expires_at(UtcDay::parse("9999-12-31").unwrap()),
        Err(EventError::Invalid)
    );
    assert_eq!(
        aggregate_alive(UtcDay::parse("2026-01-01").unwrap(), -1),
        Err(EventError::Invalid)
    );
}
#[test]
fn day_identity_is_canonical_and_valid_timestamp_bounds_do_not_wrap() {
    for bad in [
        "",
        "1969-12-31",
        "2025-02-29",
        "2024-2-29",
        "2024-02-30",
        "2024-13-01",
        "2024-02-29Z",
        "+10000-01-01",
    ] {
        assert!(UtcDay::parse(bad).is_err(), "{bad}");
    }
    assert_eq!(
        UtcDay::from_timestamp(0).unwrap(),
        UtcDay::parse("1970-01-01").unwrap()
    );
    assert_eq!(
        UtcDay::from_timestamp(MAX_EVENT_TIMESTAMP).unwrap(),
        UtcDay::parse("9999-12-31").unwrap()
    );
    assert_eq!(UtcDay::from_timestamp(-1), Err(EventError::Invalid));
    assert_eq!(
        serde_json::to_string(&UtcDay::parse("2024-02-29").unwrap()).unwrap(),
        "\"2024-02-29\""
    );
}
