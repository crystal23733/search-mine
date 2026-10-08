use liar_server::online::CommandRate;
#[test]
fn limits_burst_and_replenishes_twenty_commands_per_second_with_no_client_clock() {
    let mut rate = CommandRate::new(0);
    for _ in 0..40 {
        assert!(rate.permit(0));
    }
    assert!(!rate.permit(0));
    assert!(!rate.permit(49));
    assert!(rate.permit(50));
    assert!(!rate.permit(50));
    assert!(!rate.permit(49));
    for _ in 0..40 {
        assert!(rate.permit(2050));
    }
    assert!(!rate.permit(2050));
    for _ in 0..40 {
        assert!(rate.permit(u64::MAX));
    }
    assert!(!rate.permit(u64::MAX));
}
