use super::*;
use crate::auth::{Account, Session};
use liar_protocol::{
    game::{Outcome, PublicEndReason},
    results::{PersonalResult, ResultStats},
};
fn session() -> (Session, ExportSession) {
    let account = Uuid::new_v4();
    let revision = Uuid::new_v4();
    (
        Session {
            id: revision,
            account: Account {
                id: account,
                nickname: None,
            },
            created_at: 100,
            authenticated_at: 101,
            expires_at: 1000,
        },
        ExportSession {
            account,
            revision,
            hash: [7; 32],
        },
    )
}
fn online(n: u128) -> ExportRecord {
    let id = Uuid::from_u128(n);
    let result = StoredPersonalResult {
        account: id,
        match_id: id,
        rules_hash: "a".repeat(64),
        end_elapsed_ms: Some(100),
        reason: PublicEndReason::Timeout,
        outcome: Outcome::Draw,
        own: Some(ResultStats {
            opened_safe: 10,
            mistakes: 0,
            accusation_attempts: 0,
            correct_accusations: 0,
        }),
    }
    .project(id, id)
    .unwrap();
    ExportRecord::Online {
        result,
        recorded_at: 100,
        retention_started_at: 99,
    }
}
fn daily(n: u128) -> ExportRecord {
    ExportRecord::Daily {
        completion_id: Uuid::from_u128(n).to_string(),
        attempt_id: Uuid::new_v4().to_string(),
        metadata: liar_core::daily::DailyMetadata::derive(
            "2026-10-07",
            1,
            &liar_core::rules::RulesSnapshot::bundled(),
        )
        .unwrap(),
        opened_safe: 216,
        mistakes: 0,
        replayed_elapsed_ms: 3200,
        verified_elapsed_ms: None,
        completed_at: 100,
        replay_available: true,
    }
}
#[test]
fn own_event_projection_rejects_wrong_stream_ids_dates_time_and_expired_raw() {
    let make = |fact| ExportRecord::Event {
        event_id: Uuid::from_u128(1).to_string(),
        occurred_at: 100,
        received_at: 101,
        fact,
    };
    let match_fact = ExportEventFact::MatchEnd {
        match_id: Uuid::from_u128(2).to_string(),
        opponent: EventOpponent::Human,
        reason: PublicEndReason::Timeout,
    };
    let daily_fact = ExportEventFact::DailyVerified {
        completion_id: Uuid::from_u128(3).to_string(),
        date: "2024-02-29".into(),
        mistakes: 65535,
        trust: liar_protocol::daily::DailyTrust::VerifiedTime,
    };
    for fact in [match_fact.clone(), daily_fact.clone()] {
        let event = make(fact);
        assert_eq!(
            record_id(&event, ExportStream::Event, 101).unwrap(),
            Uuid::from_u128(1)
        );
        assert!(record_id(&event, ExportStream::Online, 101).is_err());
        assert!(
            record_id(
                &event,
                ExportStream::Event,
                100 + crate::events::RAW_EVENT_SECONDS
            )
            .is_err()
        );
    }
    for (event_id, occurred_at, received_at) in [
        (Uuid::nil(), 100, 101),
        (Uuid::from_u128(1), 100, 99),
        (Uuid::from_u128(1), 100, 102),
        (Uuid::from_u128(1), -1, 101),
    ] {
        let bad = ExportRecord::Event {
            event_id: event_id.to_string(),
            occurred_at,
            received_at,
            fact: match_fact.clone(),
        };
        assert!(record_id(&bad, ExportStream::Event, 101).is_err());
    }
    for bad in [
        ExportEventFact::MatchEnd {
            match_id: "bad".into(),
            opponent: EventOpponent::Bot,
            reason: PublicEndReason::Clear,
        },
        ExportEventFact::DailyVerified {
            completion_id: "bad".into(),
            date: "2024-02-29".into(),
            mistakes: 0,
            trust: liar_protocol::daily::DailyTrust::VerifiedCompletion,
        },
        ExportEventFact::DailyVerified {
            completion_id: Uuid::from_u128(3).to_string(),
            date: "2025-02-29".into(),
            mistakes: 0,
            trust: liar_protocol::daily::DailyTrust::VerifiedCompletion,
        },
    ] {
        assert!(record_id(&make(bad), ExportStream::Event, 101).is_err());
    }
}
#[test]
fn owner_export_rejects_a_time_shorter_than_native_completion() {
    let mut value = daily(1);
    for time in [None, Some(3200), Some(u32::MAX)] {
        if let ExportRecord::Daily {
            verified_elapsed_ms,
            ..
        } = &mut value
        {
            *verified_elapsed_ms = time;
        }
        assert!(record_id(&value, ExportStream::Daily, 100).is_ok());
    }
    if let ExportRecord::Daily {
        verified_elapsed_ms,
        ..
    } = &mut value
    {
        *verified_elapsed_ms = Some(3199);
    }
    assert!(record_id(&value, ExportStream::Daily, 100).is_err());
}
#[test]
fn proof_has_no_nickname_gate_and_rejects_identity_time_and_freshness_changes() {
    let (first, expected) = session();
    assert_eq!(proof(&first, expected, 400), Ok(()));
    assert_eq!(proof(&first, expected, 401), Err(ExportError::Reauth));
    assert_eq!(same(&first, &first, expected, 400), Ok(()));
    for change in [0, 1, 2, 3, 4, 5, 6, 7] {
        let mut bad = first.clone();
        match change {
            0 => bad.id = Uuid::nil(),
            1 => bad.id = Uuid::new_v4(),
            2 => bad.account.id = Uuid::new_v4(),
            3 => bad.created_at = 500,
            4 => bad.authenticated_at = 99,
            5 => bad.authenticated_at = 500,
            6 => bad.expires_at = 400,
            _ => bad.account.id = Uuid::nil(),
        };
        assert_eq!(proof(&bad, expected, 400), Err(ExportError::Unauthorized));
    }
    assert_eq!(
        proof(
            &first,
            ExportSession {
                account: Uuid::nil(),
                ..expected
            },
            400
        ),
        Err(ExportError::Unauthorized)
    );
    assert_eq!(proof(&first, expected, -1), Err(ExportError::Unauthorized));
    for change in [0, 1, 2] {
        let mut last = first.clone();
        match change {
            0 => last.created_at = 99,
            1 => last.authenticated_at = 102,
            _ => last.expires_at = 1001,
        };
        assert_eq!(
            same(&first, &last, expected, 400),
            Err(ExportError::Unauthorized)
        );
    }
    assert_eq!(
        same(&first, &first, expected, 1000),
        Err(ExportError::Unauthorized)
    );
}
#[test]
fn canonical_ids_and_fifty_plus_one_pages_are_strict_and_bounded() {
    let account = Uuid::new_v4();
    let valid = Uuid::new_v4().to_string();
    assert!(id(&valid).is_ok());
    for bad in [
        "junk".to_string(),
        Uuid::nil().to_string(),
        valid.to_uppercase(),
        valid.replace('-', ""),
    ] {
        assert!(id(&bad).is_err());
    }
    assert!(
        page(account, ExportStream::Online, None, vec![], 100)
            .unwrap()
            .next
            .is_none()
    );
    for count in [1, 50, 51] {
        let p = page(
            account,
            ExportStream::Online,
            None,
            (1..=count).map(online).collect(),
            100,
        )
        .unwrap();
        assert_eq!(p.records.len(), count.min(50) as usize);
        assert_eq!(p.next.is_some(), count == 51);
    }
    for records in [
        vec![online(2), online(1)],
        vec![online(1), online(1)],
        (1..=52).map(online).collect(),
    ] {
        assert!(page(account, ExportStream::Online, None, records, 100).is_err());
    }
    assert!(page(Uuid::nil(), ExportStream::Online, None, vec![], 100).is_err());
    assert!(
        page(
            account,
            ExportStream::Online,
            Some(Uuid::from_u128(1)),
            vec![online(1)],
            100
        )
        .is_err()
    );
    assert!(page(account, ExportStream::Daily, None, vec![daily(1)], 100).is_ok());
    assert!(record_id(&daily(1), ExportStream::Online, 100).is_err());
}
#[test]
fn public_online_projection_rejects_invalid_retention_and_inconsistent_result() {
    for (recorded, anchor, now) in [
        (-1, 0, 100),
        (101, 0, 100),
        (100, -1, 100),
        (100, 101, 200),
        (100, 99, 7776099),
    ] {
        let ExportRecord::Online { result, .. } = online(1) else {
            panic!()
        };
        assert!(
            record_id(
                &ExportRecord::Online {
                    result,
                    recorded_at: recorded,
                    retention_started_at: anchor
                },
                ExportStream::Online,
                now
            )
            .is_err()
        );
    }
    let ExportRecord::Online { result, .. } = online(1) else {
        panic!()
    };
    for change in [0, 1, 2, 3] {
        let mut value: PersonalResult = result.clone();
        match change {
            0 => value.match_id = "bad".into(),
            1 => value.rules_hash = "bad".into(),
            2 => value.result.completed = false,
            _ => value.own = None,
        };
        assert!(
            record_id(
                &ExportRecord::Online {
                    result: value,
                    recorded_at: 100,
                    retention_started_at: 100
                },
                ExportStream::Online,
                100
            )
            .is_err()
        );
    }
}
#[test]
fn daily_projection_rejects_wire_corruption_and_expired_records() {
    for change in 0..15 {
        let mut value = daily(1);
        let ExportRecord::Daily {
            completion_id,
            attempt_id,
            metadata,
            opened_safe,
            mistakes,
            completed_at,
            ..
        } = &mut value
        else {
            panic!()
        };
        match change {
            0 => *completion_id = "bad".into(),
            1 => *attempt_id = "bad".into(),
            2 => metadata.seed_version = 2,
            3 => metadata.rules_hash = "bad".into(),
            4 => metadata.rules_hash = "G".repeat(64),
            5 => metadata.date = "2026-02-30".into(),
            6 => metadata.mode = "wrong".into(),
            7 => metadata.seed = "01".into(),
            8 => metadata.solver_version = 0,
            9 => metadata.rng_version = 0,
            10 => *opened_safe = 0,
            11 => *opened_safe = 65536,
            12 => *mistakes = 8193,
            13 => *completed_at = 0,
            _ => *completed_at = 101,
        };
        assert!(record_id(&value, ExportStream::Daily, 100).is_err());
    }
    assert!(record_id(&daily(1), ExportStream::Daily, 604900).is_err());
    let mut value = daily(1);
    if let ExportRecord::Daily {
        replay_available, ..
    } = &mut value
    {
        *replay_available = false;
    }
    assert!(record_id(&value, ExportStream::Daily, 604900).is_ok());
    assert!(record_id(&value, ExportStream::Daily, 7776100).is_err());
}
#[test]
fn raw_replay_must_match_the_strict_canonical_payload_digest_and_receipt_metadata() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    let source = fixture["daily"]["replay"].to_string();
    let replay = decode_replay(&source).unwrap();
    let digest = Sha256::digest(serde_json::to_vec(&replay).unwrap()).to_vec();
    assert_eq!(
        raw(source.clone(), digest.clone(), replay.metadata.clone()).unwrap(),
        replay
    );
    assert!(raw("{}".into(), digest.clone(), replay.metadata.clone()).is_err());
    assert!(raw(source.clone(), vec![1; 32], replay.metadata.clone()).is_err());
    let mut other = replay.metadata;
    other.seed = "0".into();
    assert!(raw(source, digest, other).is_err());
}
