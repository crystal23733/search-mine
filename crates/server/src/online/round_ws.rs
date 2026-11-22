use super::{Context, failure, handshake, send};
use crate::online::{CommandRate, RoundConnection};
use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use liar_protocol::{
    game::{MAX_INPUT_BYTES, MAX_OUTPUT_BYTES, PublicError},
    online::OnlineError,
    rooms::ROOM_PROTOCOL_VERSION,
    rounds::{RoundEvent, RoundPayload, decode_round},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) async fn upgrade(
    State(context): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let hash = match handshake(&context, &uri, &headers) {
        Ok(hash) => hash,
        Err((status, code)) => return failure(status, code),
    };
    let Ok(_permit) = context.round_handshakes.clone().try_acquire_owned() else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, OnlineError::Capacity);
    };
    let Ok(generation) = context.registry.authorities.generation() else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, OnlineError::Unavailable);
    };
    let access = async {
        let deadline = Instant::now() + Duration::from_secs(2);
        let session = context
            .sessions
            .read_until(hash, deadline)
            .await
            .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, OnlineError::Unavailable))?
            .filter(|session| session.account.nickname.is_some())
            .ok_or((StatusCode::UNAUTHORIZED, OnlineError::Unauthorized))?;
        let account = session.account.id;
        let handle = context
            .registry
            .round_for_account(account)
            .map_err(|code| (StatusCode::CONFLICT, code))?;
        let slot = handle
            .reserve_socket(account, hash)
            .map_err(|code| (StatusCode::CONFLICT, code))?;
        let authority = context
            .registry
            .authorities
            .bind(
                generation,
                account,
                hash,
                session.expires_at,
                context.registry.auth_clock.now(),
            )
            .map_err(|code| (StatusCode::UNAUTHORIZED, code))?;
        handle
            .connect_reserved(authority, slot)
            .await
            .map_err(|code| (StatusCode::CONFLICT, code))
    };
    let connection = match tokio::time::timeout(Duration::from_secs(2), access).await {
        Ok(Ok(connection)) => connection,
        Ok(Err((status, code))) => return failure(status, code),
        Err(_) => return failure(StatusCode::SERVICE_UNAVAILABLE, OnlineError::Unavailable),
    };
    upgrade
        .max_frame_size(MAX_INPUT_BYTES)
        .max_message_size(MAX_INPUT_BYTES)
        .on_upgrade(move |socket| run(socket, connection, context))
        .into_response()
}

async fn run(mut socket: WebSocket, mut connection: RoundConnection, context: Arc<Context>) {
    let mut revoked = connection.authority().revoked();
    let mut rate = CommandRate::new(context.registry.now_ms());
    let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    let mut last_seen = Instant::now();
    let mut close = 1000;
    let mut identity: Option<(String, String, u32)> = None;
    loop {
        tokio::select! {
            biased;
            update = revoked.changed() => {
                if update.is_err() || *revoked.borrow() { close = 1008; break; }
            }
            event = connection.next() => {
                let Some(event) = event else { break; };
                if !current(&context, &connection) { close = 1008; break; }
                if !matches!(event.payload, RoundPayload::End { .. }) && !connection.current_member() { continue; }
                identity = Some((event.room_id.clone(), event.round_id.clone(), event.server_seq));
                if !send_event(&mut socket, &event).await { break; }
            }
            message = socket.recv() => {
                let message = match message {
                    Some(Ok(message)) => message,
                    Some(Err(_)) => { close = 1002; break; },
                    None => break,
                };
                let now = context.registry.now_ms();
                let result = if !rate.permit(now) {
                    Err(OnlineError::RateLimited)
                } else {
                    last_seen = Instant::now();
                    match message {
                        Message::Text(text) => decode_round(text.as_str()).map_err(public_error).and_then(|input| connection.send(input)),
                        Message::Binary(_) => Err(OnlineError::Malformed),
                        Message::Ping(payload) => { if !send(&mut socket, Message::Pong(payload)).await { break; } Ok(()) },
                        Message::Pong(_) => Ok(()),
                        Message::Close(_) => break,
                    }
                };
                if let Err(code) = result {
                    if let Some((room_id, round_id, seq)) = identity.take()
                        && current(&context, &connection) && connection.current_member()
                    {
                        let event = RoundEvent { v: ROOM_PROTOCOL_VERSION, room_id, round_id, server_seq: seq.saturating_add(1), server_time_ms: now, payload: RoundPayload::Error { code } };
                        let _ = send_event(&mut socket, &event).await;
                    }
                    close = 1008;
                    break;
                }
            }
            _ = heartbeat.tick() => {
                if !current(&context, &connection) { close = 1008; break; }
                if last_seen.elapsed() >= Duration::from_secs(45) || !send(&mut socket, Message::Ping(Vec::new().into())).await { break; }
            }
        }
    }
    let _ = send(
        &mut socket,
        Message::Close(Some(CloseFrame {
            code: close,
            reason: "".into(),
        })),
    )
    .await;
}

fn current(context: &Context, connection: &RoundConnection) -> bool {
    context
        .registry
        .authorities
        .with_authority(
            connection.authority(),
            context.registry.auth_clock.now(),
            || (),
        )
        .is_ok()
}

async fn send_event(socket: &mut WebSocket, event: &RoundEvent) -> bool {
    let Ok(text) = serde_json::to_string(event) else {
        return false;
    };
    text.len() <= MAX_OUTPUT_BYTES && send(socket, Message::Text(text.into())).await
}

fn public_error(error: PublicError) -> OnlineError {
    match error {
        PublicError::UnsupportedVersion => OnlineError::UnsupportedVersion,
        PublicError::InvalidCell => OnlineError::InvalidCell,
        _ => OnlineError::Malformed,
    }
}
