use super::*;
#[test]
fn only_known_storage_labels_decode_without_guessing() {
    for (label, value) in [
        ("clear", PublicEndReason::Clear),
        ("timeout", PublicEndReason::Timeout),
        ("forfeit", PublicEndReason::Forfeit),
        ("abandoned", PublicEndReason::Abandoned),
        ("server_failure", PublicEndReason::ServerFailure),
        ("cancelled", PublicEndReason::Cancelled),
    ] {
        assert_eq!(reason(label), Ok(value));
    }
    for (label, value) in [
        ("win", Outcome::Win),
        ("loss", Outcome::Loss),
        ("draw", Outcome::Draw),
        ("abort", Outcome::Abort),
        ("cancelled", Outcome::Cancelled),
    ] {
        assert_eq!(outcome(label), Ok(value));
    }
    for label in ["", "Win", "unknown"] {
        assert_eq!(reason(label), Err(ResultError::Unavailable));
        assert_eq!(outcome(label), Err(ResultError::Unavailable));
    }
}
