use super::{EventError, valid_timestamp};
use crate::auth::AuthLocale;
use liar_protocol::{daily::DailyTrust, game::PublicEndReason};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

pub const MAX_EVENT_BYTES: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(Uuid);
impl EventId {
    pub fn parse(value: &str) -> Result<Self, EventError> {
        Uuid::parse_str(value)
            .ok()
            .filter(|id| !id.is_nil() && id.to_string() == value)
            .map(Self)
            .ok_or(EventError::Invalid)
    }
    pub fn uuid(self) -> Uuid {
        self.0
    }
}
impl Serialize for EventId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}
impl<'de> Deserialize<'de> for EventId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?)
            .map_err(|_| serde::de::Error::custom("invalid_event_id"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LandingRoute {
    Home,
    Daily,
    Rules,
    Tutorial,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareChannel {
    Copy,
    Native,
    Png,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    Menu,
    Result,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdFailure {
    Unavailable,
    NoFill,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Essential,
    Analytics,
    Advertising,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientAction {
    LandingView {
        route: LandingRoute,
    },
    InviteOpen {},
    TutorialStart {
        run_id: EventId,
    },
    TutorialComplete {
        run_id: EventId,
        duration_ms: u32,
        skipped: bool,
    },
    RematchSelected {
        previous_match_id: EventId,
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
impl ClientAction {
    pub fn purpose(&self) -> Purpose {
        match self {
            Self::AdRequest { .. } | Self::AdImpression { .. } | Self::AdFailed { .. } => {
                Purpose::Advertising
            }
            _ => Purpose::Analytics,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientDraft {
    event_id: EventId,
    pub(super) session_key: EventId,
    pub(super) occurred_at: i64,
    locale: String,
    pub(super) event: ClientAction,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireClientDraft {
    event_version: u16,
    event_id: EventId,
    session_key: EventId,
    occurred_at: i64,
    locale: String,
    event: ClientAction,
}
pub fn decode_client(source: &[u8]) -> Result<ClientDraft, EventError> {
    if source.len() > MAX_EVENT_BYTES {
        return Err(EventError::Capacity);
    }
    let draft: WireClientDraft = serde_json::from_slice(source).map_err(|_| EventError::Invalid)?;
    if draft.event_version != 1 {
        return Err(EventError::Unsupported);
    }
    valid_timestamp(draft.occurred_at)?;
    AuthLocale::parse(&draft.locale).map_err(|_| EventError::Invalid)?;
    match &draft.event {
        ClientAction::TutorialComplete { duration_ms, .. } if *duration_ms > 3_600_000 => {
            return Err(EventError::Invalid);
        }
        ClientAction::ShareClick { date, .. } => {
            super::UtcDay::parse(date)?;
        }
        _ => {}
    }
    Ok(ClientDraft {
        event_id: draft.event_id,
        session_key: draft.session_key,
        occurred_at: draft.occurred_at,
        locale: draft.locale,
        event: draft.event,
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpponentKind {
    Human,
    Bot,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "id", rename_all = "snake_case")]
pub enum EventSubject {
    Account(EventId),
    Session(EventId),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerFact {
    MatchEnd {
        match_id: EventId,
        opponent: OpponentKind,
        reason: PublicEndReason,
    },
    DailyVerified {
        completion_id: EventId,
        date: String,
        mistakes: u16,
        trust: DailyTrust,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "source", content = "fact", rename_all = "snake_case")]
pub(super) enum EventContent {
    Client(ClientAction),
    Server(ServerFact),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProductEvent {
    event_version: u16,
    pub(super) event_id: EventId,
    pub(super) subject: EventSubject,
    pub(super) occurred_at: i64,
    locale: Option<String>,
    pub(super) content: EventContent,
}
impl ProductEvent {
    pub(super) fn optional(draft: ClientDraft) -> Self {
        Self {
            event_version: 1,
            event_id: draft.event_id,
            subject: EventSubject::Session(draft.session_key),
            occurred_at: draft.occurred_at,
            locale: Some(draft.locale),
            content: EventContent::Client(draft.event),
        }
    }
    /// The caller must bind this fact to its committed server record before collection.
    pub fn server(
        event_id: EventId,
        account: EventId,
        occurred_at: i64,
        fact: ServerFact,
    ) -> Result<Self, EventError> {
        valid_timestamp(occurred_at)?;
        if let ServerFact::DailyVerified { date, .. } = &fact {
            super::UtcDay::parse(date)?;
        }
        Ok(Self {
            event_version: 1,
            event_id,
            subject: EventSubject::Account(account),
            occurred_at,
            locale: None,
            content: EventContent::Server(fact),
        })
    }
    pub fn purpose(&self) -> Purpose {
        match &self.content {
            EventContent::Client(event) => event.purpose(),
            EventContent::Server(_) => Purpose::Essential,
        }
    }
    pub fn id(&self) -> EventId {
        self.event_id
    }
    pub fn occurred_at(&self) -> i64 {
        self.occurred_at
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DedupDecision {
    New,
    Duplicate,
    Conflict,
}
pub fn deduplicate(stored: &ProductEvent, incoming: &ProductEvent) -> DedupDecision {
    if stored.event_id != incoming.event_id {
        DedupDecision::New
    } else if stored == incoming {
        DedupDecision::Duplicate
    } else {
        DedupDecision::Conflict
    }
}

#[cfg(test)]
#[path = "../../tests/unit/event_model.rs"]
mod tests;
