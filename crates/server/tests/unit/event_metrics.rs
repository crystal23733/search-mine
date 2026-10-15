use super::*;
use crate::events::{
    ConsentSnapshot, ConsentState, EventId, Purpose, ServerFact, admit_client, decode_client,
};
use uuid::Uuid;
fn id(n: u128) -> EventId {
    EventId::parse(&Uuid::from_u128(n).to_string()).unwrap()
}
fn day(s: &str) -> UtcDay {
    UtcDay::parse(s).unwrap()
}
fn event(
    n: u128,
    account: u128,
    match_id: u128,
    at: i64,
    reason: PublicEndReason,
    opponent: OpponentKind,
) -> ProductEvent {
    ProductEvent::server(
        id(n),
        id(account),
        at,
        ServerFact::MatchEnd {
            match_id: id(match_id),
            opponent,
            reason,
        },
    )
    .unwrap()
}
fn normal(n: u128, account: u128, at: i64) -> ProductEvent {
    event(
        n,
        account,
        n + 1000,
        at,
        PublicEndReason::Clear,
        OpponentKind::Bot,
    )
}
#[test]
fn nsm_uses_distinct_account_matches_normal_reasons_and_the_exact_monday_week() {
    let week = day("2026-10-05");
    let at = week.start();
    let end = at + 7 * 86400;
    let mut events = vec![
        normal(1, 10, at),
        normal(2, 10, at + 1),
        normal(3, 20, at + 2),
        normal(4, 30, end),
        normal(5, 10, at - 1),
    ];
    events.push(events[0].clone());
    events.push(event(
        6,
        10,
        1001,
        at,
        PublicEndReason::Clear,
        OpponentKind::Bot,
    ));
    for (i, reason) in [
        PublicEndReason::Forfeit,
        PublicEndReason::Abandoned,
        PublicEndReason::ServerFailure,
        PublicEndReason::Cancelled,
    ]
    .into_iter()
    .enumerate()
    {
        events.push(event(
            100 + i as u128,
            40,
            2000 + i as u128,
            at + 3,
            reason,
            OpponentKind::Bot,
        ));
    }
    events.push(
        ProductEvent::server(
            id(50),
            id(40),
            at,
            ServerFact::DailyVerified {
                completion_id: id(60),
                date: "2026-10-05".into(),
                mistakes: 0,
                trust: liar_protocol::daily::DailyTrust::VerifiedTime,
            },
        )
        .unwrap(),
    );
    let client=decode_client(format!(r#"{{"event_version":1,"event_id":"{}","session_key":"{}","occurred_at":{at},"locale":"en","event":{{"type":"tutorial_complete","run_id":"{}","duration_ms":1,"skipped":false}}}}"#,id(70).uuid(),id(71).uuid(),id(72).uuid()).as_bytes()).unwrap();
    let consent = ConsentState::Granted {
        revision: Uuid::from_u128(1),
        valid_from: at,
        expires_at: end + 1,
    };
    let client = admit_client(
        client,
        ConsentSnapshot {
            subject: Some(id(71)),
            analytics: consent,
            advertising: consent,
        },
        at,
    )
    .unwrap();
    assert_eq!(client.purpose(), Purpose::Analytics);
    events.push(client);
    let result = weekly_nsm(&events, week, end, true).unwrap();
    assert_eq!(
        (
            result.participants,
            result.repeat_accounts,
            result.human_participations,
            result.bot_participations
        ),
        (2, 1, 0, 3)
    );
    assert!(result.complete);
    assert_eq!(result.observed_through, end);
    events.retain(|e| e.subject != EventSubject::Account(id(10)));
    assert_eq!(
        weekly_nsm(&events, week, end, true)
            .unwrap()
            .repeat_accounts,
        0
    );
    let text = serde_json::to_string(&result).unwrap();
    assert!(
        !text.contains("account_id")
            && !text.contains("session")
            && !text.contains(&id(10).uuid().to_string())
    );
}
#[test]
fn both_human_participations_are_counted_once_and_a_third_actor_or_different_fact_is_corrupt() {
    let week = day("2026-10-05");
    let at = week.start();
    let a = event(
        1,
        10,
        100,
        at,
        PublicEndReason::Timeout,
        OpponentKind::Human,
    );
    let b = event(
        2,
        20,
        100,
        at,
        PublicEndReason::Timeout,
        OpponentKind::Human,
    );
    let result = weekly_nsm(&[a.clone(), b.clone()], week, at, true).unwrap();
    assert_eq!(result.human_participations, 2);
    assert!(!result.complete);
    for other in [
        event(
            3,
            30,
            100,
            at,
            PublicEndReason::Timeout,
            OpponentKind::Human,
        ),
        event(
            4,
            30,
            100,
            at + 1,
            PublicEndReason::Timeout,
            OpponentKind::Human,
        ),
        event(5, 30, 100, at, PublicEndReason::Clear, OpponentKind::Bot),
    ] {
        assert_eq!(
            weekly_nsm(&[a.clone(), b.clone(), other], week, at + 2, true),
            Err(EventError::Conflict)
        );
    }
    let bot_a = event(1, 10, 100, at, PublicEndReason::Clear, OpponentKind::Bot);
    let bot_b = event(2, 20, 100, at, PublicEndReason::Clear, OpponentKind::Bot);
    assert_eq!(
        weekly_nsm(&[bot_a, bot_b], week, at, true),
        Err(EventError::Conflict)
    );
}
#[test]
fn metric_snapshots_reject_identity_conflicts_partial_pages_future_facts_bad_weeks_and_limits() {
    let week = day("2026-10-05");
    let at = week.start();
    let a = normal(1, 10, at);
    assert_eq!(
        weekly_nsm(&[a.clone(), normal(1, 20, at)], week, at, true),
        Err(EventError::Conflict)
    );
    assert_eq!(
        weekly_nsm(
            &[
                a.clone(),
                event(2, 10, 1001, at, PublicEndReason::Forfeit, OpponentKind::Bot)
            ],
            week,
            at,
            true
        ),
        Err(EventError::Conflict)
    );
    assert_eq!(
        weekly_nsm(std::slice::from_ref(&a), week, at, false),
        Err(EventError::Incomplete)
    );
    assert_eq!(
        weekly_nsm(&[normal(2, 20, at + 1)], week, at, true),
        Err(EventError::Invalid)
    );
    assert_eq!(
        weekly_nsm(&[], day("2026-10-06"), at, true),
        Err(EventError::Invalid)
    );
    assert_eq!(
        weekly_nsm(&[], week, at - 1, true),
        Err(EventError::Invalid)
    );
    assert_eq!(
        weekly_nsm(&[], week, i64::MAX, true),
        Err(EventError::Invalid)
    );
    assert!(weekly_nsm(&vec![a.clone(); MAX_METRIC_INPUTS], week, at, true).is_ok());
    assert_eq!(
        weekly_nsm(&vec![a; MAX_METRIC_INPUTS + 1], week, at, true),
        Err(EventError::Capacity)
    );
    let last = UtcDay::from_timestamp(super::super::MAX_EVENT_TIMESTAMP).unwrap();
    let monday = UtcDay(
        last.0
            .checked_sub_days(Days::new(u64::from(
                last.0.weekday().num_days_from_monday(),
            )))
            .unwrap(),
    );
    assert_eq!(
        weekly_nsm(&[], monday, super::super::MAX_EVENT_TIMESTAMP, true),
        Err(EventError::Invalid)
    );
}
#[test]
fn d7_uses_supplied_lifetime_first_dates_not_the_first_retained_event_and_exact_return_day() {
    let cohort = day("2026-10-05");
    let at = cohort.start();
    let end = at + 8 * 86400;
    let first = vec![
        FirstNormalCompletion {
            account: id(10),
            day: cohort,
        },
        FirstNormalCompletion {
            account: id(20),
            day: cohort,
        },
        FirstNormalCompletion {
            account: id(30),
            day: day("2026-09-01"),
        },
    ];
    let events = vec![
        normal(1, 10, at + 7 * 86400),
        normal(2, 10, at + 7 * 86400 + 1),
        normal(3, 20, at + 6 * 86400),
        normal(4, 20, end),
        normal(5, 30, at + 7 * 86400),
    ];
    let result = d7_cohort(&first, &events, cohort, end, true).unwrap();
    assert_eq!(
        (result.sample_size, result.retained, result.mature),
        (2, Some(1), true)
    );
    assert_eq!(result.rate(), Some(0.5));
    assert_eq!(result.return_day, day("2026-10-12"));
    let immature = d7_cohort(&first, &events[..3], cohort, end - 1, true).unwrap();
    assert_eq!(immature.retained, None);
    assert!(!immature.mature);
    assert_eq!(immature.rate(), None);
    let without = events
        .into_iter()
        .filter(|e| e.subject != EventSubject::Account(id(10)))
        .collect::<Vec<_>>();
    let first = first
        .into_iter()
        .filter(|e| e.account != id(10))
        .collect::<Vec<_>>();
    assert_eq!(
        d7_cohort(&first, &without, cohort, end, true)
            .unwrap()
            .retained,
        Some(0)
    );
    let empty = d7_cohort(&[], &[], cohort, end, true).unwrap();
    assert_eq!(empty.sample_size, 0);
    assert_eq!(empty.retained, Some(0));
    assert_eq!(empty.rate(), None);
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains(&id(10).uuid().to_string())
    );
}
#[test]
fn d7_refuses_missing_anchors_conflicting_first_dates_and_incomplete_or_overflowing_windows() {
    let cohort = day("2026-10-05");
    let at = cohort.start();
    let anchor = FirstNormalCompletion {
        account: id(10),
        day: cohort,
    };
    let activity = normal(1, 10, at + 7 * 86400);
    assert_eq!(
        d7_cohort(
            &[],
            std::slice::from_ref(&activity),
            cohort,
            at + 8 * 86400,
            true
        ),
        Err(EventError::Incomplete)
    );
    assert_eq!(
        d7_cohort(
            &[
                anchor,
                FirstNormalCompletion {
                    account: id(10),
                    day: day("2026-10-04")
                }
            ],
            &[],
            cohort,
            at,
            true
        ),
        Err(EventError::Conflict)
    );
    assert_eq!(
        d7_cohort(&[anchor], &[normal(2, 10, at - 1)], cohort, at, true),
        Err(EventError::Conflict)
    );
    assert_eq!(
        d7_cohort(&[anchor], &[], cohort, at, false),
        Err(EventError::Incomplete)
    );
    assert_eq!(
        d7_cohort(&[], &[], cohort, at - 1, true),
        Err(EventError::Invalid)
    );
    assert_eq!(
        d7_cohort(
            &[FirstNormalCompletion {
                account: id(10),
                day: day("2026-10-06")
            }],
            &[],
            cohort,
            at,
            true
        ),
        Err(EventError::Invalid)
    );
    assert_eq!(
        d7_cohort(&vec![anchor; MAX_METRIC_INPUTS + 1], &[], cohort, at, true),
        Err(EventError::Capacity)
    );
    assert_eq!(
        d7_cohort(
            &[],
            &[],
            day("9999-12-31"),
            super::super::MAX_EVENT_TIMESTAMP,
            true
        ),
        Err(EventError::Invalid)
    );
    assert_eq!(
        d7_cohort(&[anchor, anchor], &[activity], cohort, at + 8 * 86400, true)
            .unwrap()
            .sample_size,
        1
    );
}
