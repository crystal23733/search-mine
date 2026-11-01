use super::*;
const SUBJECT: Uuid = Uuid::from_u128(1);
fn evidence() -> VerifiedConsent {
    VerifiedConsent {
        subject: SUBJECT,
        analytics: true,
        advertising: false,
        expires_at: 2000,
    }
}
#[test]
fn grants_are_separate_current_and_bounded_by_the_authenticated_session() {
    let result = refreshed(SUBJECT, None, None, evidence(), 1000, 1500).unwrap();
    assert!(!result.revision.is_nil());
    assert!(result.allows(OptionalPurpose::Analytics, 1000));
    assert!(!result.allows(OptionalPurpose::Advertising, 1000));
    assert!(!result.allows(OptionalPurpose::Analytics, 999));
    assert!(!result.allows(OptionalPurpose::Analytics, 1500));
    assert_eq!(result.expires_at, 1500);
}
#[test]
fn withdrawal_preserves_the_other_purpose_and_old_deadline_without_reviving_late_grants() {
    let mut both = evidence();
    both.advertising = true;
    let first = refreshed(SUBJECT, None, None, both, 1000, 3000).unwrap();
    let denied = withdrawn(SUBJECT, Some(first), OptionalPurpose::Analytics, 1100, 3000).unwrap();
    assert_ne!(first.revision, denied.revision);
    assert!(!denied.allows(OptionalPurpose::Analytics, 1100));
    assert!(denied.allows(OptionalPurpose::Advertising, 1100));
    assert_eq!(denied.expires_at, first.expires_at);
    for expected in [None, Some(first.revision)] {
        assert_eq!(
            refreshed(SUBJECT, expected, Some(denied), both, 1101, 3000),
            Err(ConsentError::Conflict)
        );
    }
    let renewed = refreshed(
        SUBJECT,
        Some(denied.revision),
        Some(denied),
        both,
        1101,
        3000,
    )
    .unwrap();
    assert_ne!(renewed.revision, denied.revision);
}
#[test]
fn a_first_withdrawal_blocks_pending_initial_grants_and_never_grants_the_other_purpose() {
    for purpose in [OptionalPurpose::Analytics, OptionalPurpose::Advertising] {
        let denied = withdrawn(SUBJECT, None, purpose, 1000, 2000).unwrap();
        assert!(!denied.analytics && !denied.advertising);
        assert_eq!(
            refreshed(SUBJECT, None, Some(denied), evidence(), 1001, 2000),
            Err(ConsentError::Conflict)
        );
    }
}

#[test]
fn invalid_proofs_and_clock_windows_cannot_create_or_restore_permission() {
    let first = refreshed(SUBJECT, None, None, evidence(), 1000, 3000).unwrap();
    for (subject, expected, current, proof, now, expiry) in [
        (Uuid::nil(), None, None, evidence(), 1000, 3000),
        (SUBJECT, Some(Uuid::nil()), None, evidence(), 1000, 3000),
        (
            SUBJECT,
            None,
            None,
            VerifiedConsent {
                subject: Uuid::from_u128(9),
                ..evidence()
            },
            1000,
            3000,
        ),
        (
            SUBJECT,
            None,
            None,
            VerifiedConsent {
                expires_at: 1000,
                ..evidence()
            },
            1000,
            3000,
        ),
        (
            SUBJECT,
            None,
            None,
            VerifiedConsent {
                expires_at: MAX_EVENT_TIMESTAMP + 1,
                ..evidence()
            },
            1000,
            3000,
        ),
        (SUBJECT, None, None, evidence(), -1, 3000),
        (
            SUBJECT,
            None,
            None,
            evidence(),
            MAX_EVENT_TIMESTAMP,
            MAX_EVENT_TIMESTAMP,
        ),
        (SUBJECT, None, None, evidence(), 1000, 1000),
        (
            SUBJECT,
            None,
            None,
            evidence(),
            1000,
            MAX_EVENT_TIMESTAMP + 1,
        ),
        (
            SUBJECT,
            Some(first.revision),
            Some(CurrentConsent {
                subject: Uuid::from_u128(9),
                ..first
            }),
            evidence(),
            1000,
            3000,
        ),
        (
            SUBJECT,
            Some(Uuid::nil()),
            Some(CurrentConsent {
                revision: Uuid::nil(),
                ..first
            }),
            evidence(),
            1000,
            3000,
        ),
        (
            SUBJECT,
            Some(first.revision),
            Some(CurrentConsent {
                checked_at: -1,
                ..first
            }),
            evidence(),
            1000,
            3000,
        ),
        (
            SUBJECT,
            Some(first.revision),
            Some(first),
            evidence(),
            999,
            3000,
        ),
        (
            SUBJECT,
            Some(first.revision),
            Some(CurrentConsent {
                expires_at: 1000,
                ..first
            }),
            evidence(),
            1000,
            3000,
        ),
        (
            SUBJECT,
            Some(first.revision),
            Some(CurrentConsent {
                expires_at: MAX_EVENT_TIMESTAMP + 1,
                ..first
            }),
            evidence(),
            1000,
            3000,
        ),
    ] {
        assert_eq!(
            refreshed(subject, expected, current, proof, now, expiry),
            Err(ConsentError::Invalid)
        );
    }
    assert_eq!(
        withdrawn(SUBJECT, Some(first), OptionalPurpose::Analytics, 999, 3000),
        Err(ConsentError::Invalid)
    );
    let denied = withdrawn(
        SUBJECT,
        Some(first),
        OptionalPurpose::Advertising,
        2000,
        3000,
    )
    .unwrap();
    assert!(!denied.analytics && !denied.advertising);
    assert_eq!(denied.expires_at, 3000);
}

#[test]
fn server_clock_bounds_are_checked_without_overflow() {
    struct Clock(i64);
    impl AuthClock for Clock {
        fn now(&self) -> i64 {
            self.0
        }
    }
    for now in [-1, MAX_EVENT_TIMESTAMP + 1, i64::MAX] {
        let clock: Arc<dyn AuthClock> = Arc::new(Clock(now));
        assert_eq!(checked_now(&clock, 0), Err(ConsentError::Invalid));
    }
    let clock: Arc<dyn AuthClock> = Arc::new(Clock(1000));
    assert_eq!(checked_now(&clock, 1001), Err(ConsentError::Invalid));
    assert_eq!(checked_now(&clock, 1000), Ok(1000));
}
