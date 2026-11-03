use super::*;
fn id() -> EventId {
    EventId::parse(&Uuid::new_v4().to_string()).unwrap()
}
fn draft(tab: EventId, advertising: bool, at: i64) -> ClientDraft {
    decode_client(
        &serde_json::to_vec(&serde_json::json!({
            "event_version":1,"event_id":id(),"session_key":tab,"occurred_at":at,"locale":"ko",
            "event":if advertising { serde_json::json!({"type":"ad_request","placement":"menu"}) }
                else { serde_json::json!({"type":"landing_view","route":"home"}) }
        }))
        .unwrap(),
    )
    .unwrap()
}
fn current(owner: Uuid) -> CurrentConsent {
    CurrentConsent {
        subject: owner,
        revision: Uuid::new_v4(),
        checked_at: 1000,
        expires_at: 2000,
        analytics: true,
        advertising: false,
    }
}
#[test]
fn server_authority_allows_its_purpose_without_copying_the_auth_session_into_the_tab_key() {
    let tab = id();
    let owner = Uuid::new_v4();
    let row = current(owner);
    assert_ne!(tab.uuid(), owner);
    let accepted = admit_current(
        draft(tab, false, 1001),
        owner,
        row.revision,
        Some(row),
        1500,
    )
    .unwrap();
    assert_eq!(accepted.subject, EventSubject::Session(tab));
    assert_eq!(accepted.purpose(), Purpose::Analytics);
    assert_eq!(
        admit_current(draft(tab, true, 1001), owner, row.revision, Some(row), 1500),
        Err(EventError::Consent)
    );
    let ads = CurrentConsent {
        analytics: false,
        advertising: true,
        ..row
    };
    assert!(admit_current(draft(tab, true, 1001), owner, ads.revision, Some(ads), 1500).is_ok());
    assert_eq!(
        admit_current(
            draft(tab, false, 1001),
            owner,
            ads.revision,
            Some(ads),
            1500
        ),
        Err(EventError::Consent)
    );
}
#[test]
fn missing_foreign_stale_denied_and_expired_authority_cannot_admit_a_draft() {
    let tab = id();
    let owner = Uuid::new_v4();
    let row = current(owner);
    for value in [
        None,
        Some(CurrentConsent {
            subject: Uuid::new_v4(),
            ..row
        }),
        Some(CurrentConsent {
            analytics: false,
            ..row
        }),
        Some(CurrentConsent {
            expires_at: 1500,
            ..row
        }),
    ] {
        assert_eq!(
            admit_current(draft(tab, false, 1001), owner, row.revision, value, 1500),
            Err(EventError::Consent)
        );
    }
    assert_eq!(
        admit_current(
            draft(tab, false, 1001),
            owner,
            Uuid::new_v4(),
            Some(row),
            1500
        ),
        Err(EventError::Consent)
    );
    assert_eq!(
        admit_current(draft(tab, false, 999), owner, row.revision, Some(row), 1500),
        Err(EventError::Consent)
    );
    assert_eq!(
        admit_current(
            draft(tab, false, 1501),
            owner,
            row.revision,
            Some(row),
            1500
        ),
        Err(EventError::Invalid)
    );
    assert_eq!(
        admit_current(
            draft(tab, false, 1000),
            owner,
            row.revision,
            Some(row),
            1000 + RAW_EVENT_SECONDS
        ),
        Err(EventError::Expired)
    );
    assert_eq!(
        admit_current(
            draft(tab, false, 1001),
            Uuid::nil(),
            row.revision,
            Some(row),
            1500
        ),
        Err(EventError::Invalid)
    );
    assert_eq!(
        admit_current(draft(tab, false, 1001), owner, Uuid::nil(), Some(row), 1500),
        Err(EventError::Invalid)
    );
}
