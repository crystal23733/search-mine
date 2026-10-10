use super::*;
#[test]
fn saturation_never_evicts_live_limits_and_expiry_and_clock_are_bounded() {
    let mut rate = ResultRate::new(1);
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    for _ in 0..20 {
        rate.session([1; 32], 100).unwrap();
        rate.account(a, 100).unwrap();
    }
    assert_eq!(rate.session([1; 32], 100), Err(ResultError::RateLimited));
    assert_eq!(rate.account(a, 100), Err(ResultError::RateLimited));
    assert_eq!(rate.session([2; 32], 100), Err(ResultError::Unavailable));
    assert_eq!(rate.account(b, 100), Err(ResultError::Unavailable));
    rate.session([1; 32], 101).unwrap();
    rate.account(a, 101).unwrap();
    assert_eq!(rate.session([2; 32], 160), Err(ResultError::Unavailable));
    rate.session([2; 32], 161).unwrap();
    rate.account(b, 161).unwrap();
    assert_eq!(rate.account(a, 160), Err(ResultError::Unavailable));
    assert_eq!(rate.session([1; 32], -1), Err(ResultError::Unavailable));
}
