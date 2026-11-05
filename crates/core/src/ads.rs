//! Pure advertisement request planning. Observations are not consent or storage authority.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum AdPlacement {
    Menu,
    Result,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum AdScreen {
    Menu,
    Result,
    Countdown,
    Playing,
    Tutorial,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct AdFrequency {
    // Eligible completions only; collection and persistence belong to the adapter.
    pub completed: u32,
    pub last_requested: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(tag = "identity", rename_all = "snake_case")]
pub enum AdCounters {
    Guest {
        browser: AdFrequency,
    },
    Account {
        browser: AdFrequency,
        account: AdFrequency,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum AdCompletion {
    Match,
    Daily,
    Tutorial,
    Aborted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct AdResult {
    pub completion: AdCompletion,
    pub already_requested: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct AdOpportunity {
    pub placement: AdPlacement,
    pub screen: AdScreen,
    pub online: bool,
    pub advertising_allowed: bool,
    pub sdk_ready: bool,
    pub exclusive_slot: bool,
    pub counters: Option<AdCounters>,
    pub result: Option<AdResult>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum AdBlockReason {
    Unavailable,
    UnsafeScreen,
    SlotBusy,
    IneligibleResult,
    AlreadyRequested,
    InvalidCounter,
    FrequencyCap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdPlan {
    Blocked { reason: AdBlockReason },
    Menu,
    ReserveResult { counters: AdCounters },
}

/// A result plan must be persisted together with its per-result marker under an exclusive
/// lock before calling an SDK. Failure, no-fill, and cancellation never roll it back.
pub fn plan_ad_request(opportunity: AdOpportunity) -> AdPlan {
    use AdBlockReason::*;
    let blocked = |reason| AdPlan::Blocked { reason };
    if !opportunity.online || !opportunity.advertising_allowed || !opportunity.sdk_ready {
        return blocked(Unavailable);
    }
    if !matches!(
        (opportunity.placement, opportunity.screen),
        (AdPlacement::Menu, AdScreen::Menu) | (AdPlacement::Result, AdScreen::Result)
    ) {
        return blocked(UnsafeScreen);
    }
    if !opportunity.exclusive_slot {
        return blocked(SlotBusy);
    }
    if opportunity.placement == AdPlacement::Menu {
        return AdPlan::Menu;
    }
    let Some(result) = opportunity.result else {
        return blocked(IneligibleResult);
    };
    if !matches!(result.completion, AdCompletion::Match | AdCompletion::Daily) {
        return blocked(IneligibleResult);
    }
    if result.already_requested {
        return blocked(AlreadyRequested);
    }
    let Some(counters) = opportunity.counters else {
        return blocked(InvalidCounter);
    };
    let (browser, account) = match counters {
        AdCounters::Guest { browser } => (browser, None),
        AdCounters::Account { browser, account } => (browser, Some(account)),
    };
    if !browser.valid() || account.is_some_and(|counter| !counter.valid()) {
        return blocked(InvalidCounter);
    }
    if !browser.has_gap() || account.is_some_and(|counter| !counter.has_gap()) {
        return blocked(FrequencyCap);
    }
    let counters = match account {
        Some(account) => AdCounters::Account {
            browser: browser.reserve(),
            account: account.reserve(),
        },
        None => AdCounters::Guest {
            browser: browser.reserve(),
        },
    };
    AdPlan::ReserveResult { counters }
}

impl AdFrequency {
    fn valid(self) -> bool {
        self.last_requested
            .is_none_or(|last| (3..=self.completed).contains(&last))
    }
    fn has_gap(self) -> bool {
        self.completed - self.last_requested.unwrap_or(0) >= 3
    }
    fn reserve(self) -> Self {
        Self {
            last_requested: Some(self.completed),
            ..self
        }
    }
}
