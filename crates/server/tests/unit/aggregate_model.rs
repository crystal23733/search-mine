use super::*;
#[test]
fn an_extra_game_cannot_claim_zero_repeat_accounts() {
    let week = UtcDay::parse("2026-10-05").unwrap();
    let now = week.start() + 7 * 86400;
    let input = MetricSnapshot::WeeklyNsm(WeeklyNsm {
        week,
        observed_through: now,
        complete: true,
        participants: 2,
        repeat_accounts: 0,
        human_participations: 2,
        bot_participations: 1,
    });
    assert_eq!(AggregateReport::new(input, now), Err(EventError::Invalid));
}

fn day(value: &str) -> UtcDay {
    UtcDay::parse(value).unwrap()
}
fn weekly(now: i64) -> AggregateReport {
    let week = day("2026-10-05");
    let end = week.start() + 7 * 86400;
    AggregateReport::new(
        MetricSnapshot::WeeklyNsm(WeeklyNsm {
            week,
            observed_through: now.min(end),
            complete: now >= end,
            participants: 2,
            repeat_accounts: 1,
            human_participations: 2,
            bot_participations: 1,
        }),
        now,
    )
    .unwrap()
}
#[test]
fn original_calendar_expiry_staleness_and_nullable_d7_are_preserved() {
    let cohort = day("2026-01-31");
    let return_day = day("2026-02-07");
    let now = day("2026-02-08").start();
    let r = AggregateReport::new(
        MetricSnapshot::D7(D7Cohort {
            cohort,
            return_day,
            sample_size: 0,
            retained: Some(0),
            mature: true,
        }),
        now,
    )
    .unwrap();
    assert_eq!(r.expires_at, day("2027-02-28").start());
    assert!(!r.stale(now + 1799).unwrap());
    assert!(r.stale(now + 1800).unwrap());
    assert_eq!(r.stale(now - 1), Err(EventError::Invalid));
    assert_eq!(r.stale(r.expires_at), Err(EventError::Expired));
    assert_eq!(MetricKind::parse("d7"), Ok(MetricKind::D7));
    assert_eq!(MetricKind::D7.as_str(), "d7");
    assert_eq!(MetricKind::parse("weekly_nsm"), Ok(MetricKind::WeeklyNsm));
    assert_eq!(MetricKind::WeeklyNsm.as_str(), "weekly_nsm");
    assert_eq!(MetricKind::parse("custom"), Err(EventError::Invalid));
    let mut invalid = r.clone();
    invalid.expires_at += 1;
    assert_eq!(invalid.validate(), Err(EventError::Conflict));
    let mut invalid = r.clone();
    invalid.observed_through -= 1;
    assert_eq!(invalid.validate(), Err(EventError::Conflict));
    let mut invalid = r.clone();
    invalid.period = day("2026-01-30");
    assert_eq!(invalid.validate(), Err(EventError::Conflict));
    let mut invalid = r.clone();
    let MetricSnapshot::D7(data) = &mut invalid.snapshot else {
        panic!()
    };
    data.retained = Some(1);
    assert_eq!(invalid.validate(), Err(EventError::Invalid));
    let mut invalid = r.clone();
    let MetricSnapshot::D7(data) = &mut invalid.snapshot else {
        panic!()
    };
    data.return_day = cohort;
    assert_eq!(invalid.validate(), Err(EventError::Invalid));
    let immature = AggregateReport::new(
        MetricSnapshot::D7(D7Cohort {
            cohort,
            return_day,
            sample_size: 1,
            retained: None,
            mature: false,
        }),
        now - 1,
    )
    .unwrap();
    assert!(!immature.snapshot.complete());
    let mut invalid = immature.clone();
    let MetricSnapshot::D7(data) = &mut invalid.snapshot else {
        panic!()
    };
    data.retained = Some(0);
    assert_eq!(invalid.validate(), Err(EventError::Invalid));
}
#[test]
fn equal_clock_conflicts_regressions_and_invalid_counters_cannot_overwrite_reports() {
    let start = day("2026-10-05").start();
    let first = weekly(start + 10);
    assert_eq!(first.can_replace(&first), Ok(false));
    let later = weekly(start + 11);
    assert_eq!(first.can_replace(&later), Ok(true));
    assert_eq!(later.can_replace(&first), Err(EventError::Conflict));
    let mut same = first.clone();
    let MetricSnapshot::WeeklyNsm(data) = &mut same.snapshot else {
        panic!()
    };
    data.bot_participations += 1;
    assert_eq!(first.can_replace(&same), Err(EventError::Conflict));
    let complete = weekly(start + 7 * 86400);
    assert_eq!(first.can_replace(&complete), Ok(true));
    assert_eq!(complete.can_replace(&first), Err(EventError::Conflict));
    let mut invalid = first.clone();
    let MetricSnapshot::WeeklyNsm(data) = &mut invalid.snapshot else {
        panic!()
    };
    data.repeat_accounts = 3;
    assert_eq!(invalid.validate(), Err(EventError::Invalid));
    let mut invalid = first.clone();
    let MetricSnapshot::WeeklyNsm(data) = &mut invalid.snapshot else {
        panic!()
    };
    data.participants = 10001;
    assert_eq!(invalid.validate(), Err(EventError::Invalid));
    let mut invalid = first.clone();
    let MetricSnapshot::WeeklyNsm(data) = &mut invalid.snapshot else {
        panic!()
    };
    data.human_participations = u32::MAX;
    assert_eq!(invalid.validate(), Err(EventError::Invalid));
    let mut invalid = first.clone();
    let MetricSnapshot::WeeklyNsm(data) = &mut invalid.snapshot else {
        panic!()
    };
    data.human_participations = 0;
    data.bot_participations = 0;
    assert_eq!(invalid.validate(), Err(EventError::Invalid));
    let mut invalid = first.clone();
    let MetricSnapshot::WeeklyNsm(data) = &mut invalid.snapshot else {
        panic!()
    };
    data.observed_through -= 1;
    assert_eq!(invalid.validate(), Err(EventError::Invalid));
    let mut invalid = first.clone();
    let MetricSnapshot::WeeklyNsm(data) = &mut invalid.snapshot else {
        panic!()
    };
    data.week = day("2026-10-06");
    invalid.computed_at = data.week.start() + 10;
    invalid.observed_through = invalid.computed_at;
    data.observed_through = invalid.observed_through;
    invalid.period = data.week;
    invalid.expires_at = aggregate_expires_at(data.week).unwrap();
    assert_eq!(invalid.validate(), Err(EventError::Invalid));
    assert_eq!(
        AggregateReport::new(first.snapshot.clone(), start - 1),
        Err(EventError::Invalid)
    );
}

#[test]
fn utc_week_and_the_twenty_three_slot_rotation_are_closed_and_bounded() {
    let now = day("2026-10-21").start() + 86399;
    for cursor in 0..=22 {
        let targets = aggregate_schedule(now, cursor).unwrap();
        assert_eq!(targets[0], (MetricKind::WeeklyNsm, day("2026-10-19")));
        if cursor == 0 {
            assert_eq!(targets[1], (MetricKind::WeeklyNsm, day("2026-10-12")));
        } else {
            assert_eq!(targets[1].0, MetricKind::D7);
            assert_eq!(
                targets[1].1.start(),
                day("2026-10-21").start() - (i64::from(cursor) + 7) * 86400
            );
        }
    }
    for (at, cursor) in [(now, 23), (-1, 0), (253402300800, 0), (0, 0)] {
        assert_eq!(aggregate_schedule(at, cursor), Err(EventError::Invalid));
    }
}
