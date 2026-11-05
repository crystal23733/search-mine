use liar_core::ads::*;

fn result(completed: u32, last_requested: Option<u32>) -> AdOpportunity {
    AdOpportunity {
        placement: AdPlacement::Result,
        screen: AdScreen::Result,
        online: true,
        advertising_allowed: true,
        sdk_ready: true,
        exclusive_slot: true,
        counters: Some(AdCounters::Guest {
            browser: AdFrequency {
                completed,
                last_requested,
            },
        }),
        result: Some(AdResult {
            completion: AdCompletion::Match,
            already_requested: false,
        }),
    }
}
fn blocked(reason: AdBlockReason) -> AdPlan {
    AdPlan::Blocked { reason }
}

#[test]
fn first_third_then_sixth_completion_consumes_requests_even_without_an_impression() {
    let mut last = None;
    for completed in 0..=6 {
        let plan = plan_ad_request(result(completed, last));
        if completed == 3 || completed == 6 {
            let next = AdCounters::Guest {
                browser: AdFrequency {
                    completed,
                    last_requested: Some(completed),
                },
            };
            assert_eq!(plan, AdPlan::ReserveResult { counters: next });
            // Persist the reservation before the SDK, even when it subsequently has no fill.
            last = Some(completed);
            assert_eq!(
                plan_ad_request(result(completed, last)),
                blocked(AdBlockReason::FrequencyCap)
            );
        } else {
            assert_eq!(plan, blocked(AdBlockReason::FrequencyCap));
        }
    }
}

#[test]
fn menu_is_a_separate_slot_without_consuming_result_frequency() {
    let mut input = result(0, None);
    input.placement = AdPlacement::Menu;
    input.screen = AdScreen::Menu;
    input.counters = None;
    input.result = None;
    assert_eq!(plan_ad_request(input), AdPlan::Menu);
    assert_eq!(input.counters, None);
}

#[test]
fn account_and_browser_must_both_have_a_gap_in_their_own_completion_ordinals() {
    for (browser, account) in [(2, 30), (30, 2)] {
        let mut input = result(3, None);
        input.counters = Some(AdCounters::Account {
            browser: AdFrequency {
                completed: browser,
                last_requested: None,
            },
            account: AdFrequency {
                completed: account,
                last_requested: None,
            },
        });
        assert_eq!(plan_ad_request(input), blocked(AdBlockReason::FrequencyCap));
    }
    let mut input = result(3, None);
    input.counters = Some(AdCounters::Account {
        browser: AdFrequency {
            completed: 6,
            last_requested: Some(3),
        },
        account: AdFrequency {
            completed: 40,
            last_requested: Some(37),
        },
    });
    assert_eq!(
        plan_ad_request(input),
        AdPlan::ReserveResult {
            counters: AdCounters::Account {
                browser: AdFrequency {
                    completed: 6,
                    last_requested: Some(6)
                },
                account: AdFrequency {
                    completed: 40,
                    last_requested: Some(40)
                },
            }
        }
    );
}

#[test]
fn every_unsafe_screen_and_wrong_placement_is_blocked_even_after_enough_completions() {
    for placement in [AdPlacement::Menu, AdPlacement::Result] {
        for screen in [
            AdScreen::Menu,
            AdScreen::Result,
            AdScreen::Countdown,
            AdScreen::Playing,
            AdScreen::Tutorial,
            AdScreen::Other,
        ] {
            if matches!(
                (placement, screen),
                (AdPlacement::Menu, AdScreen::Menu) | (AdPlacement::Result, AdScreen::Result)
            ) {
                continue;
            }
            let mut input = result(9, None);
            input.placement = placement;
            input.screen = screen;
            assert_eq!(plan_ad_request(input), blocked(AdBlockReason::UnsafeScreen));
        }
    }
}

#[test]
fn purpose_connection_sdk_and_exclusive_slot_are_checked_for_both_placements() {
    for placement in [AdPlacement::Menu, AdPlacement::Result] {
        let mut safe = result(9, None);
        safe.placement = placement;
        safe.screen = if placement == AdPlacement::Menu {
            AdScreen::Menu
        } else {
            AdScreen::Result
        };
        for gate in 0..4 {
            let mut input = safe;
            match gate {
                0 => input.online = false,
                1 => input.advertising_allowed = false,
                2 => input.sdk_ready = false,
                _ => input.exclusive_slot = false,
            }
            assert_eq!(
                plan_ad_request(input),
                blocked(if gate == 3 {
                    AdBlockReason::SlotBusy
                } else {
                    AdBlockReason::Unavailable
                })
            );
        }
    }
}

#[test]
fn only_completed_match_or_daily_with_unreserved_result_is_eligible() {
    for completion in [AdCompletion::Match, AdCompletion::Daily] {
        let mut input = result(3, None);
        input.result.as_mut().unwrap().completion = completion;
        assert!(matches!(
            plan_ad_request(input),
            AdPlan::ReserveResult { .. }
        ));
        input.result.as_mut().unwrap().already_requested = true;
        assert_eq!(
            plan_ad_request(input),
            blocked(AdBlockReason::AlreadyRequested)
        );
    }
    for completion in [AdCompletion::Tutorial, AdCompletion::Aborted] {
        let mut input = result(3, None);
        input.result.as_mut().unwrap().completion = completion;
        assert_eq!(
            plan_ad_request(input),
            blocked(AdBlockReason::IneligibleResult)
        );
    }
    let mut input = result(3, None);
    input.result = None;
    assert_eq!(
        plan_ad_request(input),
        blocked(AdBlockReason::IneligibleResult)
    );
}

#[test]
fn missing_or_impossible_counters_fail_closed_without_subtraction_wraparound() {
    let mut input = result(3, None);
    input.counters = None;
    assert_eq!(
        plan_ad_request(input),
        blocked(AdBlockReason::InvalidCounter)
    );
    for last in [0, 1, 2, 4, u32::MAX] {
        let invalid = AdFrequency {
            completed: 3,
            last_requested: Some(last),
        };
        for counters in [
            AdCounters::Guest { browser: invalid },
            AdCounters::Account {
                browser: AdFrequency {
                    completed: 9,
                    last_requested: None,
                },
                account: invalid,
            },
            AdCounters::Account {
                browser: invalid,
                account: AdFrequency {
                    completed: 9,
                    last_requested: None,
                },
            },
        ] {
            input.counters = Some(counters);
            assert_eq!(
                plan_ad_request(input),
                blocked(AdBlockReason::InvalidCounter)
            );
        }
    }
    assert!(matches!(
        plan_ad_request(result(u32::MAX, Some(u32::MAX - 3))),
        AdPlan::ReserveResult { .. }
    ));
    assert_eq!(
        plan_ad_request(result(u32::MAX, Some(u32::MAX))),
        blocked(AdBlockReason::FrequencyCap)
    );
}

proptest::proptest! {
    #[test]
    fn result_reservation_requires_each_gap_and_consumes_both_counters(
        browser in 3..=u32::MAX, account in 3..=u32::MAX,
        browser_last in 0..=u32::MAX, account_last in 0..=u32::MAX,
    ) {
        let browser_last = browser_last % (browser - 2) + 3;
        let account_last = account_last % (account - 2) + 3;
        let mut input = result(3, None);
        input.counters = Some(AdCounters::Account {
            browser: AdFrequency { completed: browser, last_requested: Some(browser_last) },
            account: AdFrequency { completed: account, last_requested: Some(account_last) },
        });
        match plan_ad_request(input) {
            AdPlan::ReserveResult { counters } => {
                proptest::prop_assert!(browser - browser_last >= 3);
                proptest::prop_assert!(account - account_last >= 3);
                input.counters = Some(counters);
                proptest::prop_assert_eq!(plan_ad_request(input), blocked(AdBlockReason::FrequencyCap));
            }
            plan => {
                proptest::prop_assert!(browser - browser_last < 3 || account - account_last < 3);
                proptest::prop_assert_eq!(plan, blocked(AdBlockReason::FrequencyCap));
            }
        }
    }
}
