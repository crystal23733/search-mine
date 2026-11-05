use serde_json::{Value, json};

pub fn cases() -> Vec<Value> {
    use liar_core::ads::*;
    let base = AdOpportunity {
        placement: AdPlacement::Result,
        screen: AdScreen::Result,
        online: true,
        advertising_allowed: true,
        sdk_ready: true,
        exclusive_slot: true,
        counters: None,
        result: Some(AdResult {
            completion: AdCompletion::Match,
            already_requested: false,
        }),
    };
    let mut cases = Vec::new();
    let mut append = |name: String, opportunity: AdOpportunity| {
        let input = json!({"v":1,"opportunity":opportunity}).to_string();
        cases.push(
            json!({"name":name,"input":input,"expected":{"ok":plan_ad_request(opportunity)}}),
        );
    };
    for (last, start) in [(None, 0), (Some(3), 3)] {
        for completed in start..=6 {
            append(
                format!("completed-{completed}-last-{last:?}"),
                AdOpportunity {
                    counters: Some(AdCounters::Guest {
                        browser: AdFrequency {
                            completed,
                            last_requested: last,
                        },
                    }),
                    ..base
                },
            );
        }
    }
    let eligible = AdOpportunity {
        counters: Some(AdCounters::Account {
            browser: AdFrequency {
                completed: 6,
                last_requested: Some(3),
            },
            account: AdFrequency {
                completed: 40,
                last_requested: Some(37),
            },
        }),
        ..base
    };
    append("account-and-browser".into(), eligible);
    for screen in [
        AdScreen::Menu,
        AdScreen::Result,
        AdScreen::Countdown,
        AdScreen::Playing,
        AdScreen::Tutorial,
        AdScreen::Other,
    ] {
        for placement in [AdPlacement::Menu, AdPlacement::Result] {
            append(
                format!("screen-{screen:?}-{placement:?}"),
                AdOpportunity {
                    screen,
                    placement,
                    ..eligible
                },
            );
        }
    }
    for gate in 0..4 {
        let mut input = eligible;
        match gate {
            0 => input.online = false,
            1 => input.advertising_allowed = false,
            2 => input.sdk_ready = false,
            _ => input.exclusive_slot = false,
        }
        append(format!("closed-gate-{gate}"), input);
    }
    for completion in [
        AdCompletion::Match,
        AdCompletion::Daily,
        AdCompletion::Tutorial,
        AdCompletion::Aborted,
    ] {
        for already_requested in [false, true] {
            append(
                format!("completion-{completion:?}-{already_requested}"),
                AdOpportunity {
                    result: Some(AdResult {
                        completion,
                        already_requested,
                    }),
                    ..eligible
                },
            );
        }
    }
    for (browser, account) in [(2, 30), (30, 2)] {
        append(
            format!("conservative-{browser}-{account}"),
            AdOpportunity {
                counters: Some(AdCounters::Account {
                    browser: AdFrequency {
                        completed: browser,
                        last_requested: None,
                    },
                    account: AdFrequency {
                        completed: account,
                        last_requested: None,
                    },
                }),
                ..base
            },
        );
    }
    append(
        "invalid-counter".into(),
        AdOpportunity {
            counters: Some(AdCounters::Guest {
                browser: AdFrequency {
                    completed: 3,
                    last_requested: Some(4),
                },
            }),
            ..base
        },
    );
    append("missing-counter".into(), base);
    append(
        "missing-result".into(),
        AdOpportunity {
            result: None,
            ..eligible
        },
    );
    let valid = json!({"v":1,"opportunity":eligible}).to_string();
    for input in [
        "{}".into(),
        valid.replacen("\"v\":1", "\"v\":2", 1),
        valid.replacen("\"v\":1", "\"v\":1,\"v\":1", 1),
        format!("{} ", " ".repeat(2048)),
        valid.replace("\"result\"", "\"unknown\""),
    ] {
        let error = if input.contains("\"v\":2") {
            "unsupported_version"
        } else {
            "malformed"
        };
        cases.push(json!({"name":format!("invalid-{}",cases.len()),"input":input,"expected":{"error":error}}));
    }
    cases
}
