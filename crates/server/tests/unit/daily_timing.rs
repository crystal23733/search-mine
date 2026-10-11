use super::*;
use crate::daily::{NativeReplayVerifier, ReplayVerifier};
use liar_core::rules::RulesSnapshot;
fn definition() -> PublishedDaily {
    PublishedDaily::new("2026-10-07", 1, RulesSnapshot::bundled()).unwrap()
}
fn evidence() -> ReplayEvidence {
    let value: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    NativeReplayVerifier
        .verify(&definition(), &value["daily"]["replay"].to_string())
        .unwrap()
}
fn timing() -> TimingAttempt {
    TimingAttempt::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        &definition(),
        1000,
    )
    .unwrap()
}
#[test]
fn heartbeat_duplicate_does_not_extend_last_seen_and_gap_is_permanent() {
    let mut state = timing();
    let ack = state.heartbeat(1, 16000).unwrap().unwrap();
    assert_eq!(
        ack,
        HeartbeatAck {
            sequence: 1,
            elapsed_ms: 15000
        }
    );
    assert_eq!(state.heartbeat(1, 59000).unwrap(), Some(ack));
    assert_eq!(state.heartbeat(2, 61000).unwrap(), None);
    assert_eq!(state.heartbeat(2, 61001).unwrap(), None);
    assert_eq!(state.finish(&evidence(), 62000).unwrap(), None);
    let mut exact = timing();
    assert!(exact.heartbeat(1, 45999).unwrap().is_some());
    assert_eq!(exact.heartbeat(2, 90999).unwrap(), None);
    let mut first = timing();
    assert_eq!(first.heartbeat(1, 46000).unwrap(), None);
}
#[test]
fn zero_skipped_and_old_sequences_are_rejected_without_refreshing_deadline() {
    let mut state = timing();
    assert_eq!(state.heartbeat(0, 1100), Err(TicketError::Invalid));
    assert_eq!(state.heartbeat(2, 1200), Err(TicketError::Conflict));
    state.heartbeat(1, 16000).unwrap();
    state.heartbeat(2, 31000).unwrap();
    assert_eq!(state.heartbeat(1, 32000), Err(TicketError::Conflict));
    state.last_sequence = u32::MAX;
    state.last_ack = Some(HeartbeatAck {
        sequence: u32::MAX,
        elapsed_ms: 30000,
    });
    assert!(state.heartbeat(u32::MAX, 33000).unwrap().is_some());
    assert_eq!(state.heartbeat(1, 34000), Err(TicketError::Conflict));
}
#[test]
fn server_elapsed_is_required_and_finished_digest_and_time_are_immutable() {
    let proof = evidence();
    let mut state = timing();
    let now = 1000 + state.countdown + u64::from(proof.replayed_elapsed_ms());
    let saved = state.finish(&proof, now).unwrap().unwrap();
    assert_eq!(saved.elapsed_ms(), proof.replayed_elapsed_ms());
    assert!(saved.matches(state.account, state.grant.attempt, &proof));
    assert!(!saved.matches(Uuid::new_v4(), state.grant.attempt, &proof));
    assert!(!saved.matches(state.account, Uuid::new_v4(), &proof));
    assert_eq!(
        state.finish(&proof, now + 50000).unwrap(),
        Some(saved.clone())
    );
    assert_eq!(state.heartbeat(1, now + 50001).unwrap(), None);
    let value: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    let mut changed = value["daily"]["replay"].clone();
    changed["final_time_ms"] = serde_json::json!(u32::MAX);
    let changed = NativeReplayVerifier
        .verify(&definition(), &changed.to_string())
        .unwrap();
    assert!(!saved.matches(state.account, state.grant.attempt, &changed));
    assert_eq!(
        state.finish(&changed, now + 50002),
        Err(TicketError::Conflict)
    );
    assert_eq!(state.finish(&proof, state.expires).unwrap(), None);
    assert!(!format!("{saved:?}").contains(&state.account.to_string()));
}
#[test]
fn early_finish_countdown_and_exact_ttl_never_create_a_time_proof() {
    let proof = evidence();
    for now in [
        1000,
        3999,
        4000,
        4000 + u64::from(proof.replayed_elapsed_ms()) - 1,
    ] {
        let mut state = timing();
        assert_eq!(state.finish(&proof, now).unwrap(), None);
        assert_eq!(state.finish(&proof, 15000).unwrap(), None);
    }
    let mut state = timing();
    for sequence in 1..=16 {
        state
            .heartbeat(sequence, 1000 + u64::from(sequence) * 15000)
            .unwrap();
    }
    let expires = state.expires;
    assert!(state.finish(&proof, expires - 1).unwrap().is_some());
    assert_eq!(state.finish(&proof, expires).unwrap(), None);
    let mut state = timing();
    assert_eq!(state.finish(&proof, state.expires).unwrap(), None);
}
#[test]
fn metadata_mismatch_is_invalid_and_rules_snapshot_controls_deadline() {
    let proof = evidence();
    let mut rules = RulesSnapshot::bundled();
    rules.rules.countdown_ms += 1;
    let rules = RulesSnapshot::from_rules(rules.rules).unwrap();
    let other = PublishedDaily::new("2026-10-07", 1, rules).unwrap();
    let mut state =
        TimingAttempt::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), &other, 1000).unwrap();
    assert_eq!(state.grant.metadata, *other.metadata());
    assert_eq!(state.grant.ttl_ms, 243001);
    assert_eq!(state.finish(&proof, 15000), Err(TicketError::Invalid));
}
#[test]
fn invalid_identity_overflow_and_clock_reversal_fail_closed() {
    assert!(matches!(
        TimingAttempt::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::nil(),
            &definition(),
            0
        ),
        Err(TicketError::Invalid)
    ));
    assert!(matches!(
        TimingAttempt::new(
            Uuid::nil(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            &definition(),
            0
        ),
        Err(TicketError::Invalid)
    ));
    assert!(matches!(
        TimingAttempt::new(
            Uuid::new_v4(),
            Uuid::nil(),
            Uuid::new_v4(),
            &definition(),
            0
        ),
        Err(TicketError::Invalid)
    ));
    assert!(matches!(
        TimingAttempt::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            &definition(),
            u64::MAX
        ),
        Err(TicketError::Unavailable)
    ));
    let mut state = timing();
    state.heartbeat(1, 16000).unwrap();
    assert_eq!(state.heartbeat(2, 15999), Err(TicketError::Unavailable));
    assert_eq!(state.finish(&evidence(), 16001).unwrap(), None);
    let mut state = timing();
    assert_eq!(
        state.finish(&evidence(), 999),
        Err(TicketError::Unavailable)
    );
}

#[test]
fn clock_reversal_discards_even_an_already_finished_policy_proof() {
    let proof = evidence();
    let mut state = timing();
    assert!(state.finish(&proof, 15000).unwrap().is_some());
    assert_eq!(state.finish(&proof, 14999), Err(TicketError::Unavailable));
    assert_eq!(state.finish(&proof, 15001).unwrap(), None);
}
