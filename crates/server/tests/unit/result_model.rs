use super::*;
fn row() -> StoredPersonalResult {
    StoredPersonalResult {
        account: Uuid::new_v4(),
        match_id: Uuid::new_v4(),
        rules_hash: "a".repeat(64),
        end_elapsed_ms: 42,
        reason: PublicEndReason::Timeout,
        outcome: Outcome::Draw,
        own: ResultStats {
            opened_safe: 256,
            mistakes: 4096,
            accusation_attempts: 4096,
            correct_accusations: 4096,
        },
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
            6 => r.end_elapsed_ms = 9007199254740992,
            7 => r.own.opened_safe = 257,
            8 => r.own.mistakes = 4097,
            9 => r.own.accusation_attempts = 4097,
            10 => r.own.correct_accusations = 4097,
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
