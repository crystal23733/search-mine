use super::*;
use liar_protocol::{
    game::{GameResult, PublicEndReason},
    results::{PersonalResult, ResultStats},
};
fn row(id: u128, at: i64, outcome: Outcome, opponent: MeOpponent) -> MeOnlineRow {
    let reason = match outcome {
        Outcome::Abort => PublicEndReason::ServerFailure,
        Outcome::Cancelled => PublicEndReason::Cancelled,
        _ => PublicEndReason::Timeout,
    };
    MeOnlineRow {
        key: MeKey {
            primary: at * 1000000,
            secondary: at * 1000000,
            id: Uuid::from_u128(id),
        },
        entry: MeOnlineRecord {
            result: PersonalResult {
                match_id: Uuid::from_u128(id).to_string(),
                rules_hash: "a".repeat(64),
                end_elapsed_ms: Some(240000),
                result: GameResult {
                    reason,
                    outcome,
                    completed: reason.completed(),
                },
                own: Some(ResultStats {
                    opened_safe: 8,
                    mistakes: 0,
                    accusation_attempts: 0,
                    correct_accusations: 0,
                }),
            },
            opponent,
            retention_started_at: at,
            recorded_at: at,
        },
    }
}
fn data(rows: Vec<MeOnlineRow>) -> MeData {
    MeData {
        account: MeAccount {
            id: Uuid::from_u128(8).to_string(),
            nickname: None,
            created_at: 1,
            last_seen_at: 1,
            identities: vec![],
        },
        online: rows,
        daily: vec![],
        consent: None,
    }
}
#[test]
fn summary_uses_the_full_filtered_period_and_excludes_abort_from_the_denominator() {
    let rows = vec![
        row(1, 1000, Outcome::Win, MeOpponent::Human),
        row(2, 1000, Outcome::Loss, MeOpponent::Human),
        row(3, 1000, Outcome::Draw, MeOpponent::Bot),
        row(4, 1000, Outcome::Abort, MeOpponent::Unknown),
        row(5, 1000, Outcome::Cancelled, MeOpponent::Bot),
    ];
    let value = summary(&rows).unwrap();
    assert_eq!(
        (
            value.wins,
            value.losses,
            value.draws,
            value.aborted,
            value.cancelled,
            value.played
        ),
        (1, 1, 1, 1, 1, 3)
    );
    assert_eq!(value.win_rate, Some(1.0 / 3.0));
    assert_eq!(summary(&[]).unwrap().win_rate, None);
}
#[test]
fn pagination_is_newest_first_twenty_items_while_the_summary_includes_all_twenty_one() {
    let rows = (1..=21)
        .map(|i| row(i, 1000, Outcome::Win, MeOpponent::Human))
        .collect();
    let query = MeQuery::History {
        kind: MeKind::Online,
        opponent: MeOpponentFilter::All,
        after: None,
    };
    let (payload, _) = build(data(rows), query, 1100).unwrap();
    let MePayload::History(page) = payload else {
        panic!("history");
    };
    assert_eq!(page.items.len(), 20);
    assert_eq!(page.total, 21);
    assert_eq!(page.online.unwrap().played, 21);
    let next = MeQuery::history(MeHistoryInput {
        v: 1,
        kind: MeKind::Online,
        opponent: MeOpponentFilter::All,
        cursor: page.next,
    })
    .unwrap();
    let rows = (1..=21)
        .map(|i| row(i, 1000, Outcome::Win, MeOpponent::Human))
        .collect();
    let (MePayload::History(last), _) = build(data(rows), next, 1100).unwrap() else {
        panic!("history");
    };
    assert_eq!(last.items.len(), 1);
    assert!(last.next.is_none());
}
#[test]
fn consent_views_do_not_forge_cmp_availability_and_expired_choices_are_unknown() {
    let view = consent_view(None, 1000).unwrap();
    assert!(!view.cmp_available);
    assert_eq!(view.status, MeConsentStatus::Unknown);
    let value = CurrentConsent {
        subject: Uuid::from_u128(1),
        revision: Uuid::from_u128(2),
        checked_at: 1000,
        expires_at: 2000,
        analytics: true,
        advertising: false,
    };
    let current = consent_view(Some(value), 1999).unwrap();
    assert_eq!(current.analytics, MeChoice::Granted);
    assert_eq!(current.advertising, MeChoice::Denied);
    let expired = consent_view(Some(value), 2000).unwrap();
    assert_eq!(expired.status, MeConsentStatus::Expired);
    assert_eq!(expired.analytics, MeChoice::Unknown);
}

fn daily_row(id: u128, at: i64) -> MeDailyRow {
    MeDailyRow {
        key: MeKey {
            primary: at * 1000000,
            secondary: at * 1000000,
            id: Uuid::from_u128(id),
        },
        entry: MeDailyRecord {
            completion_id: Uuid::from_u128(id).to_string(),
            date: "2026-10-01".into(),
            opened_safe: 100,
            mistakes: 0,
            replayed_elapsed_ms: 1000,
            verified_elapsed_ms: None,
            trust: liar_protocol::daily::DailyTrust::VerifiedCompletion,
            completed_at: at,
        },
    }
}
#[test]
fn overview_daily_filters_retention_and_expiry_preserve_unknowns() {
    let at = 1000;
    let now = at + ME_RETENTION_SECONDS;
    let mut value = data(vec![
        row(1, at, Outcome::Win, MeOpponent::Human),
        row(2, at + 1, Outcome::Abort, MeOpponent::Unknown),
        row(3, now + 1, Outcome::Win, MeOpponent::Bot),
    ]);
    value.online[1].entry.result.own = None;
    value.daily = vec![
        daily_row(1, at),
        daily_row(2, at + 2),
        daily_row(3, now + 1),
    ];
    value.consent = Some(CurrentConsent {
        subject: Uuid::from_u128(9),
        revision: Uuid::from_u128(10),
        checked_at: now,
        expires_at: now + 10,
        analytics: false,
        advertising: false,
    });
    let (MePayload::Overview(view), until) = build(value, MeQuery::Overview, now).unwrap() else {
        panic!("overview")
    };
    assert_eq!(view.online.aborted, 1);
    assert_eq!(view.online.played, 0);
    assert_eq!(view.online.win_rate, None);
    assert_eq!(view.recent_online.len(), 1);
    assert_eq!(view.recent_online[0].result.own, None);
    assert_eq!(view.daily_total, 1);
    assert_eq!(until, now + 1);
    let mut value = data(vec![]);
    value.daily = (1..=21).map(|id| daily_row(id, now)).collect();
    let (MePayload::History(page), _) = build(
        value,
        MeQuery::History {
            kind: MeKind::Daily,
            opponent: MeOpponentFilter::All,
            after: None,
        },
        now,
    )
    .unwrap() else {
        panic!("daily")
    };
    assert_eq!(page.total, 21);
    assert!(page.online.is_none());
    assert!(page.next.is_some());
    assert_eq!(page.items.len(), 20);
    for filter in [MeOpponentFilter::Human, MeOpponentFilter::Bot] {
        let value = data(vec![
            row(1, 1000, Outcome::Win, MeOpponent::Human),
            row(2, 1000, Outcome::Loss, MeOpponent::Bot),
            row(3, 1000, Outcome::Draw, MeOpponent::Unknown),
        ]);
        let (MePayload::History(page), _) = build(
            value,
            MeQuery::History {
                kind: MeKind::Online,
                opponent: filter,
                after: None,
            },
            1100,
        )
        .unwrap() else {
            panic!("history")
        };
        assert_eq!(page.total, 1);
        assert_eq!(page.online.unwrap().played, 1);
    }
    let mut value = data(vec![row(1, at, Outcome::Win, MeOpponent::Human)]);
    value.online[0].key.primary += 1;
    value.online[0].key.secondary += 1;
    let (_, until) = build(value, MeQuery::Overview, now).unwrap();
    assert_eq!(until, now + 1);
}
#[test]
fn opaque_cursors_are_canonical_bounded_and_tied_to_the_selected_stream() {
    let key = MeKey {
        primary: 1000000000,
        secondary: 1000000001,
        id: Uuid::from_u128(9),
    };
    let token = cursor(MeKind::Online, MeOpponentFilter::Bot, key).unwrap();
    let input = |cursor| MeHistoryInput {
        v: 1,
        kind: MeKind::Online,
        opponent: MeOpponentFilter::Bot,
        cursor: Some(cursor),
    };
    assert!(
        matches!(MeQuery::history(input(token.clone())).unwrap(),MeQuery::History{after:Some(k),..}if k==key)
    );
    for value in [
        String::new(),
        "a".repeat(513),
        "%%%".into(),
        format!("{token}="),
        URL_SAFE_NO_PAD.encode(b"{}"),
        cursor(MeKind::Daily, MeOpponentFilter::All, key).unwrap(),
    ] {
        assert!(MeQuery::history(input(value)).is_err());
    }
    for (primary, secondary, id, v) in [
        (-1, 1, Uuid::from_u128(1).to_string(), 1),
        (2, 1, Uuid::from_u128(1).to_string(), 1),
        (
            1,
            MAX_ME_TIMESTAMP * 1000000 + 1,
            Uuid::from_u128(1).to_string(),
            1,
        ),
        (1, 1, Uuid::nil().to_string(), 1),
        (1, 1, "bad".into(), 1),
        (1, 1, Uuid::from_u128(1).to_string(), 2),
    ] {
        let value = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&Cursor {
                v,
                kind: MeKind::Online,
                opponent: MeOpponentFilter::Bot,
                primary,
                secondary,
                id,
            })
            .unwrap(),
        );
        assert!(MeQuery::history(input(value)).is_err());
    }
    let mut wrong = input(token);
    wrong.v = 2;
    assert!(MeQuery::history(wrong).is_err());
    assert!(
        MeQuery::history(MeHistoryInput {
            v: 1,
            kind: MeKind::Daily,
            opponent: MeOpponentFilter::Human,
            cursor: None
        })
        .is_err()
    );
}
#[test]
fn malformed_domain_records_fail_closed_instead_of_becoming_empty_history() {
    assert!(
        summary(&vec![
            row(1, 1000, Outcome::Win, MeOpponent::Human);
            MAX_ME_RECORDS + 1
        ])
        .is_err()
    );
    let mut broken = row(1, 1000, Outcome::Win, MeOpponent::Human);
    broken.entry.result.result.completed = false;
    assert!(summary(&[broken]).is_err());
    for change in 0..8 {
        let mut value = data(vec![row(1, 1000, Outcome::Win, MeOpponent::Human)]);
        match change {
            0 => value.account.id = "bad".into(),
            1 => value.account.created_at = -1,
            2 => value.account.last_seen_at = 2000,
            3 => value.online[0].key.id = Uuid::nil(),
            4 => value.online[0].entry.recorded_at = 999,
            5 => value.online.push(value.online[0].clone()),
            6 => value.online = vec![value.online[0].clone(); MAX_ME_RECORDS + 1],
            _ => value.daily = vec![daily_row(1, 1000); MAX_ME_RECORDS + 1],
        }
        assert!(build(value, MeQuery::Overview, 1100).is_err());
    }
    for change in 0..3 {
        let mut value = data(vec![]);
        value.daily = vec![daily_row(1, 1000)];
        match change {
            0 => value.daily[0].entry.date = "bad".into(),
            1 => value.daily[0].key.secondary += 1,
            _ => value.daily.push(value.daily[0].clone()),
        };
        assert!(build(value, MeQuery::Overview, 1100).is_err());
    }
    assert!(build(data(vec![]), MeQuery::Overview, -1).is_err());
    for (kind, opponent, after) in [
        (MeKind::Daily, MeOpponentFilter::Bot, None),
        (
            MeKind::Online,
            MeOpponentFilter::All,
            Some(MeKey {
                primary: 1200000000,
                secondary: 1200000000,
                id: Uuid::from_u128(9),
            }),
        ),
    ] {
        assert!(
            build(
                data(vec![]),
                MeQuery::History {
                    kind,
                    opponent,
                    after
                },
                1100
            )
            .is_err()
        );
    }
    assert!(
        consent_view(
            Some(CurrentConsent {
                subject: Uuid::nil(),
                revision: Uuid::nil(),
                checked_at: 1000,
                expires_at: 2000,
                analytics: true,
                advertising: true
            }),
            1100
        )
        .is_err()
    );
}
#[test]
fn session_views_allow_old_authentication_but_reject_swaps_expiry_and_backwards_time() {
    let value = Session {
        id: Uuid::from_u128(1),
        account: crate::auth::Account {
            id: Uuid::from_u128(2),
            nickname: None,
        },
        created_at: 1000,
        authenticated_at: 1000,
        expires_at: 2000,
    };
    assert!(valid_session(&value, 1500));
    assert!(same_session(&value, &value, 1500));
    assert!(!valid_session(&value, 2000));
    assert!(!valid_session(&value, 999));
    let mut changed = value.clone();
    changed.id = Uuid::new_v4();
    assert!(!same_session(&value, &changed, 1500));
}
