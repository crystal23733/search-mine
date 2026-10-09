use super::*;
use crate::auth::BrowserCookies;
use axum::{
    Json, Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, Uri, header},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use liar_protocol::{
    game::{MAX_INPUT_BYTES, MAX_OUTPUT_BYTES, PublicError},
    online::{OnlineError, OnlinePayload, decode_online},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

struct Context {
    origin: String,
    sessions: Arc<dyn SessionReader>,
    registry: Arc<MatchRegistry>,
    connections: Arc<Semaphore>,
}
pub fn websocket_router(
    origin: &str,
    sessions: Arc<dyn SessionReader>,
    registry: Arc<MatchRegistry>,
    connections: usize,
) -> Result<Router, OnlineError> {
    let url = url::Url::parse(origin).map_err(|_| OnlineError::Malformed)?;
    if connections == 0
        || connections > 20000
        || url.scheme() != "https"
        || !matches!(url.host(), Some(url::Host::Domain(_)))
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(OnlineError::Malformed);
    }
    let context = Arc::new(Context {
        origin: url.origin().ascii_serialization(),
        sessions,
        registry,
        connections: Arc::new(Semaphore::new(connections)),
    });
    Ok(Router::new()
        .route("/api/v1/ws", get(upgrade))
        .layer(middleware::map_response(
            |mut response: Response| async move {
                response.headers_mut().insert(
                    header::CACHE_CONTROL,
                    "no-store".parse().expect("Constant header"),
                );
                response
            },
        ))
        .with_state(context))
}
fn failure(status: StatusCode, code: OnlineError) -> Response {
    (status, Json(serde_json::json!({"error":code}))).into_response()
}
async fn upgrade(
    State(context): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if headers.get_all(header::ORIGIN).iter().count() != 1
        || headers.get(header::ORIGIN).and_then(|h| h.to_str().ok())
            != Some(context.origin.as_str())
    {
        return failure(StatusCode::FORBIDDEN, OnlineError::Unauthorized);
    }
    if uri.query().is_some() || headers.contains_key(header::SEC_WEBSOCKET_PROTOCOL) {
        return failure(StatusCode::BAD_REQUEST, OnlineError::Malformed);
    }
    let hash = match BrowserCookies::parse(&headers)
        .ok()
        .and_then(|cookies| cookies.session)
    {
        Some(token) => token.hash(),
        None => return failure(StatusCode::UNAUTHORIZED, OnlineError::Unauthorized),
    };
    let Ok(permit) = context.connections.clone().try_acquire_owned() else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, OnlineError::Capacity);
    };
    let Ok(generation) = context.registry.authorities.generation() else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, OnlineError::Unavailable);
    };
    let session =
        match tokio::time::timeout(Duration::from_secs(2), context.sessions.read(hash)).await {
            Ok(Ok(Some(session))) if session.account.nickname.is_some() => session,
            Ok(Ok(_)) => return failure(StatusCode::UNAUTHORIZED, OnlineError::Unauthorized),
            _ => return failure(StatusCode::SERVICE_UNAVAILABLE, OnlineError::Unavailable),
        };
    let handle = match context.registry.for_account(session.account.id) {
        Ok(handle) => handle,
        Err(code) => return failure(StatusCode::CONFLICT, code),
    };
    let authority = match context.registry.authorities.bind(
        generation,
        session.account.id,
        hash,
        session.expires_at,
        context.registry.auth_clock.now(),
    ) {
        Ok(authority) => authority,
        Err(code) => return failure(StatusCode::UNAUTHORIZED, code),
    };
    let connection = match handle.connect(authority).await {
        Ok(connection) => connection,
        Err(code) => return failure(StatusCode::CONFLICT, code),
    };
    upgrade
        .max_frame_size(MAX_INPUT_BYTES)
        .max_message_size(MAX_INPUT_BYTES)
        .on_upgrade(move |socket| run(socket, connection, context, permit))
        .into_response()
}
async fn send(socket: &mut WebSocket, message: Message) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_secs(2), socket.send(message)).await,
        Ok(Ok(()))
    )
}
async fn run(
    mut socket: WebSocket,
    mut connection: MatchConnection,
    context: Arc<Context>,
    _permit: OwnedSemaphorePermit,
) {
    let mut revoked = connection.authority().revoked();
    let mut rate = CommandRate::new(context.registry.clock.now_ms());
    let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    let mut last_seen = Instant::now();
    let mut close_code = 1000;
    loop {
        tokio::select! {
            biased;
            changed = revoked.changed() => { if changed.is_err() || *revoked.borrow() { close_code = 1008; break; } }
            outgoing = connection.next() => {
                let Some(event) = outgoing else { break; };
                if context.registry.authorities.with_authority(connection.authority(), context.registry.auth_clock.now(), || ()).is_err() { break; }
                let Ok(text) = serde_json::to_string(&event) else { break; };
                if text.len() > MAX_OUTPUT_BYTES || !send(&mut socket, Message::Text(text.into())).await { break; }
            }
            incoming = socket.recv() => {
                let message = match incoming { Some(Ok(message)) => message, Some(Err(_)) => { close_code = 1002; break; }, None => break };
                if !rate.permit(context.registry.clock.now_ms()) {
                    close_code = 1008;
                    let _ = send_error(&mut socket, &mut connection, &context, OnlineError::RateLimited).await;
                    break;
                }
                last_seen = Instant::now();
                match message {
                    Message::Text(text) => {
                        let input = match decode_online(text.as_str()) {
                            Ok(input) => input,
                            Err(error) => {
                                let code = match error { PublicError::UnsupportedVersion => OnlineError::UnsupportedVersion, PublicError::InvalidCell => OnlineError::InvalidCell, _ => OnlineError::Malformed };
                                close_code = 1008;
                                let _ = send_error(&mut socket, &mut connection, &context, code).await;
                                break;
                            }
                        };
                        if let Err(code) = connection.send(input) { close_code = 1008; let _ = send_error(&mut socket, &mut connection, &context, code).await; break; }
                    }
                    Message::Ping(payload) => { if !send(&mut socket, Message::Pong(payload)).await { break; } }
                    Message::Pong(_) => {}
                    Message::Close(_) => break,
                    Message::Binary(_) => { close_code = 1008; let _ = send_error(&mut socket, &mut connection, &context, OnlineError::Malformed).await; break; }
                }
            }
            _ = heartbeat.tick() => {
                if last_seen.elapsed() >= Duration::from_secs(45) || context.registry.authorities.with_authority(connection.authority(), context.registry.auth_clock.now(), || ()).is_err() || !send(&mut socket, Message::Ping(Vec::new().into())).await { break; }
            }
        }
    }
    let _ = send(
        &mut socket,
        Message::Close(Some(CloseFrame {
            code: close_code,
            reason: "".into(),
        })),
    )
    .await;
}
async fn send_error(
    socket: &mut WebSocket,
    connection: &mut MatchConnection,
    context: &Context,
    code: OnlineError,
) -> bool {
    if connection.reject(code).is_err() {
        return false;
    }
    tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(event) = connection.next().await {
            if context
                .registry
                .authorities
                .with_authority(
                    connection.authority(),
                    context.registry.auth_clock.now(),
                    || (),
                )
                .is_err()
            {
                return false;
            }
            let is_error = matches!(event.payload, OnlinePayload::Error { .. });
            let Ok(text) = serde_json::to_string(&event) else {
                return false;
            };
            if text.len() > MAX_OUTPUT_BYTES || !send(socket, Message::Text(text.into())).await {
                return false;
            }
            if is_error {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false)
}
