use super::{Bytes, Context};
use crate::auth::{AuthError, BrowserCookies};
use axum::{
    Json,
    extract::{State, rejection::BytesRejection},
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use liar_protocol::{
    lobby::LobbyErrorCode,
    online::OnlineError,
    rooms::{
        MAX_ROOM_OUTPUT_BYTES, ROOM_PROTOCOL_VERSION, RoomIntent, RoomPayload, RoomResponse,
        RoomWireError, decode_room_request,
    },
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

type Failure = (StatusCode, RoomWireError);

fn failure(error: RoomWireError) -> Failure {
    let status = match error {
        RoomWireError::Malformed | RoomWireError::UnsupportedVersion => StatusCode::BAD_REQUEST,
        RoomWireError::Unauthorized => StatusCode::UNAUTHORIZED,
        RoomWireError::Forbidden => StatusCode::FORBIDDEN,
        RoomWireError::NotFound => StatusCode::NOT_FOUND,
        RoomWireError::Busy
        | RoomWireError::Full
        | RoomWireError::NotReady
        | RoomWireError::Conflict
        | RoomWireError::Stale => StatusCode::CONFLICT,
        RoomWireError::Capacity | RoomWireError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        RoomWireError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
    };
    (status, error)
}

fn wire(status: StatusCode, server_time_ms: u64, payload: RoomPayload) -> Response {
    let response = RoomResponse {
        v: ROOM_PROTOCOL_VERSION,
        server_time_ms,
        payload,
    };
    if let Ok(bytes) = serde_json::to_vec(&response)
        && bytes.len() <= MAX_ROOM_OUTPUT_BYTES
    {
        return (status, [(header::CONTENT_TYPE, "application/json")], bytes).into_response();
    }
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(RoomResponse {
            v: ROOM_PROTOCOL_VERSION,
            server_time_ms,
            payload: RoomPayload::Error {
                error: RoomWireError::Unavailable,
            },
        }),
    )
        .into_response()
}

pub(super) async fn disabled() -> Response {
    wire(
        StatusCode::SERVICE_UNAVAILABLE,
        0,
        RoomPayload::Error {
            error: RoomWireError::Unavailable,
        },
    )
}

fn rate(error: LobbyErrorCode) -> Failure {
    failure(if error == LobbyErrorCode::RateLimited {
        RoomWireError::RateLimited
    } else {
        RoomWireError::Unavailable
    })
}

fn authority(error: OnlineError) -> Failure {
    failure(match error {
        OnlineError::Unauthorized => RoomWireError::Unauthorized,
        OnlineError::Capacity => RoomWireError::Capacity,
        _ => RoomWireError::Unavailable,
    })
}

pub(super) async fn handle(
    State(ctx): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let method = async {
        if headers.get_all(header::ORIGIN).iter().count() != 1
            || headers
                .get(header::ORIGIN)
                .and_then(|value| value.to_str().ok())
                != Some(ctx.security.origin())
        {
            return Err(failure(RoomWireError::Forbidden));
        }
        if uri.query().is_some()
            || headers.get_all(header::CONTENT_TYPE).iter().count() != 1
            || headers
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(';').next())
                .map(str::trim)
                != Some("application/json")
        {
            return Err(failure(RoomWireError::Malformed));
        }
        let body = body.map_err(|cause| (cause.status(), RoomWireError::Malformed))?;
        let input = decode_room_request(&body).map_err(failure)?;
        let now = ctx.authentication.clock.now();
        if let RoomIntent::List { cursor } = input.intent {
            let _permit = ctx
                .public_requests
                .clone()
                .try_acquire_owned()
                .map_err(|_| failure(RoomWireError::Capacity))?;
            ctx.rate
                .lock()
                .map_err(|_| failure(RoomWireError::Unavailable))?
                .public_list(now)
                .map_err(rate)?;
            let page = ctx.service.room_list(cursor.as_ref()).map_err(failure)?;
            return Ok(RoomPayload::List { page });
        }
        let cookies =
            BrowserCookies::parse(&headers).map_err(|_| failure(RoomWireError::Malformed))?;
        ctx.security
            .require(&headers, &cookies, now)
            .map_err(|cause| {
                if cause == AuthError::Unavailable {
                    failure(RoomWireError::Unavailable)
                } else {
                    failure(RoomWireError::Forbidden)
                }
            })?;
        let token = cookies
            .session
            .ok_or_else(|| failure(RoomWireError::Unauthorized))?;
        let hash = token.hash();
        let _permit = ctx
            .requests
            .clone()
            .try_acquire_owned()
            .map_err(|_| failure(RoomWireError::Capacity))?;
        ctx.rate
            .lock()
            .map_err(|_| failure(RoomWireError::Unavailable))?
            .session(hash, now)
            .map_err(rate)?;
        let generation = ctx
            .authentication
            .authorities
            .generation()
            .map_err(authority)?;
        let session = ctx
            .sessions
            .read_until(hash, Instant::now() + Duration::from_secs(2))
            .await
            .map_err(authority)?
            .ok_or_else(|| failure(RoomWireError::Unauthorized))?;
        let nickname = session
            .account
            .nickname
            .as_ref()
            .ok_or_else(|| failure(RoomWireError::Unauthorized))?;
        let now = ctx.authentication.clock.now();
        let owner = ctx
            .authentication
            .authorities
            .bind_shared(
                generation,
                session.account.id,
                hash,
                session.expires_at,
                now,
            )
            .map_err(authority)?;
        ctx.rate
            .lock()
            .map_err(|_| failure(RoomWireError::Unavailable))?
            .account(
                session.account.id,
                matches!(input.intent, RoomIntent::Join { .. }),
                now,
            )
            .map_err(rate)?;
        let room = ctx
            .service
            .room_command(&owner, nickname, input.intent)
            .map_err(failure)?;
        Ok(RoomPayload::Current { room })
    };
    let result = tokio::time::timeout(Duration::from_secs(2), method)
        .await
        .unwrap_or_else(|_| Err(failure(RoomWireError::Unavailable)));
    let (status, payload) = match result {
        Ok(payload) => (StatusCode::OK, payload),
        Err((status, error)) => (status, RoomPayload::Error { error }),
    };
    wire(status, ctx.service.now_ms(), payload)
}
