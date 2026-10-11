use super::*;
use liar_core::rules::RulesSnapshot;

#[test]
fn receipt_requires_a_complete_published_board_and_valid_server_time() {
    let published = PublishedDaily::new("2026-10-07", 1, RulesSnapshot::bundled()).unwrap();
    let valid = DailyReceipt {
        id: Uuid::new_v4(),
        attempt: Uuid::new_v4(),
        metadata: published.metadata().clone(),
        opened_safe: 216,
        mistakes: 0,
        replayed_elapsed_ms: 3200,
        completed_at: 100,
    };
    assert_eq!(valid.validate(&published, 100), Ok(()));
    let mut variants = vec![];
    let mut invalid = valid.clone();
    invalid.id = Uuid::nil();
    variants.push(invalid);
    let mut invalid = valid.clone();
    invalid.attempt = Uuid::nil();
    variants.push(invalid);
    let mut invalid = valid.clone();
    invalid.metadata.seed = "1".into();
    variants.push(invalid);
    let mut invalid = valid.clone();
    invalid.opened_safe = 215;
    variants.push(invalid);
    let mut invalid = valid.clone();
    invalid.mistakes = published.rules().rules.max_commands_per_seat + 1;
    variants.push(invalid);
    let mut invalid = valid.clone();
    invalid.completed_at = 0;
    variants.push(invalid);
    let mut invalid = valid.clone();
    invalid.completed_at = 101;
    variants.push(invalid);
    for invalid in variants {
        assert_eq!(
            invalid.validate(&published, 100),
            Err(DailyStoreError::Unavailable)
        );
    }
    assert!(!valid.expired(99));
    assert!(!valid.expired(100 + 7776000 - 1));
    assert!(valid.expired(100 + 7776000));
    assert!(valid.expired(i64::MAX));
}
