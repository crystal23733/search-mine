use super::{LobbyError, LobbyServiceError, LobbyStatus, LobbyView, OpponentKind};
use liar_protocol::{lobby::*, online::OnlineError};
pub(super) fn code(error: LobbyServiceError) -> LobbyErrorCode {
    match error {
        LobbyServiceError::Policy(error) => match error {
            LobbyError::Invalid => LobbyErrorCode::Malformed,
            LobbyError::Busy => LobbyErrorCode::Busy,
            LobbyError::NotFound => LobbyErrorCode::NotFound,
            LobbyError::Full => LobbyErrorCode::Full,
            LobbyError::Stale => LobbyErrorCode::Stale,
            LobbyError::Capacity => LobbyErrorCode::Capacity,
            LobbyError::InvalidTime | LobbyError::Collision => LobbyErrorCode::Unavailable,
        },
        LobbyServiceError::Online(error) => match error {
            OnlineError::Unauthorized => LobbyErrorCode::Unauthorized,
            OnlineError::Capacity => LobbyErrorCode::Capacity,
            OnlineError::Malformed | OnlineError::InvalidCell => LobbyErrorCode::Malformed,
            OnlineError::RateLimited => LobbyErrorCode::RateLimited,
            OnlineError::UnsupportedVersion => LobbyErrorCode::UnsupportedVersion,
            _ => LobbyErrorCode::Unavailable,
        },
    }
}
pub(super) fn difficulty(value: liar_core::bot::Difficulty) -> LobbyDifficulty {
    match value {
        liar_core::bot::Difficulty::Easy => LobbyDifficulty::Easy,
        liar_core::bot::Difficulty::Normal => LobbyDifficulty::Normal,
        liar_core::bot::Difficulty::Hard => LobbyDifficulty::Hard,
    }
}
pub(super) fn opponent(value: OpponentKind) -> LobbyOpponent {
    match value {
        OpponentKind::Human => LobbyOpponent::Human,
        OpponentKind::Bot => LobbyOpponent::Bot,
    }
}
pub(super) fn view(value: LobbyView) -> LobbyState {
    match value {
        LobbyView::Idle => LobbyState::Idle,
        LobbyView::Matched {
            match_id,
            own_seat,
            opponent: kind,
        } => LobbyState::Matched {
            match_id: match_id.to_string(),
            own_seat: own_seat as u8,
            opponent: opponent(kind),
        },
        LobbyView::Failed(error) => LobbyState::Failed {
            error: code(error.into()),
        },
        LobbyView::Waiting(value) => match value {
            LobbyStatus::Queued {
                id,
                deadline,
                difficulty: level,
            } => LobbyState::Queued {
                queue_id: id.to_string(),
                deadline_ms: deadline,
                difficulty: difficulty(level),
            },
            LobbyStatus::Room {
                id,
                code,
                own_seat,
                occupied,
                ready,
                expires_at,
            } => LobbyState::Room {
                room_id: id.to_string(),
                code: code.as_str().into(),
                own_seat: own_seat as u8,
                occupied,
                ready,
                expires_at_ms: expires_at,
            },
            LobbyStatus::Preparing {
                key,
                opponent: kind,
            } => LobbyState::Preparing {
                identity: LobbyCancellation::Preparing {
                    entity: key.entity.to_string(),
                    generation: key.generation.to_string(),
                },
                opponent: opponent(kind),
            },
        },
    }
}
