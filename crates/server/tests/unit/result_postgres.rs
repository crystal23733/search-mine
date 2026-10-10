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

#[test]
fn all_null_patterns_and_negative_statistics_fail_closed_without_defaults() {
    for mask in 0..16 {
        let values = std::array::from_fn(|i| if mask & (1 << i) == 0 { Some(1) } else { None });
        let result = statistics(values);
        match mask {
            0 => assert_eq!(result.unwrap().unwrap().opened_safe, 1),
            15 => assert_eq!(result.unwrap(), None),
            _ => assert_eq!(result, Err(ResultError::Unavailable)),
        }
    }
    for i in 0..4 {
        let mut values = [Some(1); 4];
        values[i] = Some(-1);
        assert_eq!(statistics(values), Err(ResultError::Unavailable));
    }
}
