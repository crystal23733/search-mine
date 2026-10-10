use super::*;
fn row() -> StoredPersonalResult {
    StoredPersonalResult {
        account: Uuid::new_v4(),
        match_id: Uuid::new_v4(),
        rules_hash: "a".repeat(64),
        end_elapsed_ms: Some(42),
        reason: PublicEndReason::Timeout,
        outcome: Outcome::Draw,
        own: Some(ResultStats {
            opened_safe: 256,
            mistakes: 4096,
            accusation_attempts: 4096,
            correct_accusations: 4096,
        }),
    }
}
#[test]
fn completed_classification_and_minimal_projection_preserve_historical_hash() {
    for (reason, outcome, completed) in [
        (PublicEndReason::Clear, Outcome::Win, true),
        (PublicEndReason::Timeout, Outcome::Loss, true),
        (PublicEndReason::Forfeit, Outcome::Win, true),
        (PublicEndReason::Abandoned, Outcome::Draw, true),
        (PublicEndReason::ServerFailure, Outcome::Abort, false),
        (PublicEndReason::Cancelled, Outcome::Cancelled, false),
    ] {
        let mut record = row();
        record.reason = reason;
        record.outcome = outcome;
        let projected = record
            .clone()
            .project(record.account, record.match_id)
            .unwrap();
        assert_eq!(
            serde_json::to_value(projected).unwrap(),
            serde_json::json!({
                "match_id": record.match_id.to_string(), "rules_hash":"a".repeat(64), "end_elapsed_ms":42,
                "result":{"reason":reason,"outcome":outcome,"completed":completed},
                "own":{"opened_safe":256,"mistakes":4096,"accusation_attempts":4096,"correct_accusations":4096}
            })
        );
    }
}
#[test]
fn wrong_subject_corruption_and_impossible_outcomes_never_project() {
    let initial = row();
    let mut variants = vec![];
    for field in 0..13 {
        let mut r = initial.clone();
        match field {
            0 => r.account = Uuid::new_v4(),
            1 => r.match_id = Uuid::new_v4(),
            2 => r.account = Uuid::nil(),
            3 => r.match_id = Uuid::nil(),
            4 => r.rules_hash = "A".repeat(64),
            5 => r.rules_hash = "0".repeat(63),
            6 => r.end_elapsed_ms = Some(9007199254740992),
            7 => r.own.as_mut().unwrap().opened_safe = 257,
            8 => r.own.as_mut().unwrap().mistakes = 4097,
            9 => r.own.as_mut().unwrap().accusation_attempts = 4097,
            10 => r.own.as_mut().unwrap().correct_accusations = 4097,
            11 => r.outcome = Outcome::Abort,
            _ => r.reason = PublicEndReason::Cancelled,
        }
        variants.push(r);
    }
    for r in variants {
        assert_eq!(
            r.project(initial.account, initial.match_id),
            Err(ResultError::Unavailable)
        );
    }
}

#[test]
fn unknown_details_require_a_complete_failure_abort_without_fabricating_numbers() {
    let mut r = row();
    r.reason = PublicEndReason::ServerFailure;
    r.outcome = Outcome::Abort;
    r.end_elapsed_ms = None;
    r.own = None;
    let value = serde_json::to_value(r.clone().project(r.account, r.match_id).unwrap()).unwrap();
    assert!(value["own"].is_null());
    assert!(value["end_elapsed_ms"].is_null());
    assert_eq!(value["result"]["completed"], false);
    for case in 0..6 {
        let mut bad = r.clone();
        match case {
            0 => bad.end_elapsed_ms = Some(0),
            1 => bad.own = row().own,
            2 => bad.reason = PublicEndReason::Timeout,
            3 => bad.reason = PublicEndReason::Cancelled,
            4 => bad.outcome = Outcome::Draw,
            _ => bad.outcome = Outcome::Cancelled,
        }
        assert_eq!(
            bad.project(r.account, r.match_id),
            Err(ResultError::Unavailable)
        );
    }
}
