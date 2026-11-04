//! Closed optional product event wire contracts; current permission is server-owned.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LandingRoute {
    Home,
    Daily,
    Rules,
    Tutorial,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ShareChannel {
    Copy,
    Native,
    Png,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    Menu,
    Result,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AdFailure {
    Unavailable,
    NoFill,
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionalEventAction<I> {
    LandingView {
        route: LandingRoute,
    },
    InviteOpen {},
    TutorialStart {
        run_id: I,
    },
    TutorialComplete {
        run_id: I,
        duration_ms: u32,
        skipped: bool,
    },
    RematchSelected {
        previous_match_id: I,
    },
    ShareClick {
        date: String,
        channel: ShareChannel,
    },
    AdRequest {
        placement: Placement,
    },
    AdImpression {
        placement: Placement,
    },
    AdFailed {
        placement: Placement,
        reason: AdFailure,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OptionalEventPurpose {
    Analytics,
    Advertising,
}
impl<I> OptionalEventAction<I> {
    pub fn collection_purpose(&self) -> OptionalEventPurpose {
        match self {
            Self::AdRequest { .. } | Self::AdImpression { .. } | Self::AdFailed { .. } => {
                OptionalEventPurpose::Advertising
            }
            _ => OptionalEventPurpose::Analytics,
        }
    }
}
/// Generate the browser contract from the same purpose method used at write time.
pub fn optional_event_purposes() -> std::collections::BTreeMap<String, OptionalEventPurpose> {
    let actions = [
        OptionalEventAction::LandingView {
            route: LandingRoute::Home,
        },
        OptionalEventAction::InviteOpen {},
        OptionalEventAction::TutorialStart {
            run_id: String::new(),
        },
        OptionalEventAction::TutorialComplete {
            run_id: String::new(),
            duration_ms: 0,
            skipped: false,
        },
        OptionalEventAction::RematchSelected {
            previous_match_id: String::new(),
        },
        OptionalEventAction::ShareClick {
            date: String::new(),
            channel: ShareChannel::Copy,
        },
        OptionalEventAction::AdRequest {
            placement: Placement::Menu,
        },
        OptionalEventAction::AdImpression {
            placement: Placement::Menu,
        },
        OptionalEventAction::AdFailed {
            placement: Placement::Menu,
            reason: AdFailure::Unavailable,
        },
    ];
    actions
        .into_iter()
        .map(|action| {
            let tag = serde_json::to_value(&action).expect("Event contract serializes")["type"]
                .as_str()
                .expect("Event tag")
                .to_owned();
            (tag, action.collection_purpose())
        })
        .collect()
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct OptionalEventDraft<I = String> {
    pub event_version: u16,
    pub event_id: I,
    pub session_key: I,
    #[ts(type = "number")]
    pub occurred_at: i64,
    pub locale: String,
    pub event: OptionalEventAction<I>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct OptionalEventCaptureInput {
    pub v: u16,
    pub expected_revision: String,
    pub draft: OptionalEventDraft,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct OptionalEventReceipt {
    pub v: u16,
    pub event_id: String,
    #[ts(type = "number")]
    pub received_at: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OptionalEventApiError {
    Malformed,
    UnsupportedVersion,
    Unauthorized,
    Consent,
    Conflict,
    RateLimited,
    Unavailable,
}
pub const MAX_OPTIONAL_EVENT_BYTES: usize = 2048;
pub fn decode_optional_capture(
    source: &[u8],
) -> Result<OptionalEventCaptureInput, OptionalEventApiError> {
    if source.len() > MAX_OPTIONAL_EVENT_BYTES {
        return Err(OptionalEventApiError::Malformed);
    }
    let value: OptionalEventCaptureInput =
        serde_json::from_slice(source).map_err(|_| OptionalEventApiError::Malformed)?;
    if value.v != crate::game::PROTOCOL_VERSION {
        return Err(OptionalEventApiError::UnsupportedVersion);
    }
    let id = uuid::Uuid::parse_str(&value.expected_revision)
        .map_err(|_| OptionalEventApiError::Malformed)?;
    if id.is_nil() || id.to_string() != value.expected_revision {
        return Err(OptionalEventApiError::Malformed);
    }
    Ok(value)
}
pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        LandingRoute::decl(config),
        ShareChannel::decl(config),
        Placement::decl(config),
        AdFailure::decl(config),
        OptionalEventPurpose::decl(config),
        OptionalEventAction::<String>::decl(config),
        OptionalEventDraft::<String>::decl(config),
        OptionalEventCaptureInput::decl(config),
        OptionalEventReceipt::decl(config),
        OptionalEventApiError::decl(config),
    ]
}
