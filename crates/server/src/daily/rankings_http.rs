use super::{
    boundary::{Failure, observe},
    http::{failure, no_store, rate},
    rankings::empty_rankings,
    *,
};
use crate::{
    auth::{AuthClock, BrowserSecurity},
    results::ResultRate,
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State, rejection::BytesRejection},
    http::{HeaderMap, Uri, header},
    middleware,
    response::{IntoResponse, Response},
    routing::post,
};
use liar_protocol::daily::DailyRankingsInput;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;

struct Context {
    reader: Arc<dyn DailyRankingReader>,
    workers: BoundedReplayVerifier,
    clock: Arc<dyn AuthClock>,
    security: Arc<BrowserSecurity>,
    requests: Arc<Semaphore>,
    rate: Mutex<ResultRate>,
}
pub fn daily_rankings_router(
    reader: Arc<dyn DailyRankingReader>,
    workers: BoundedReplayVerifier,
    clock: Arc<dyn AuthClock>,
    security: Arc<BrowserSecurity>,
) -> Router {
    Router::new()
        .route("/api/v1/daily/rankings", post(read))
        .layer(DefaultBodyLimit::max(4096))
        .layer(middleware::map_response(no_store))
        .with_state(Arc::new(Context {
            reader,
            workers,
            clock,
            security,
            requests: Arc::new(Semaphore::new(2)),
            rate: Mutex::new(ResultRate::new(1)),
        }))
}
async fn read(
    State(ctx): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let result = async {
        if uri.query().is_some()
            || headers.get_all(header::ORIGIN).iter().count() != 1
            || headers.get(header::ORIGIN).and_then(|v| v.to_str().ok())
                != Some(ctx.security.origin())
            || headers.get_all(header::CONTENT_TYPE).iter().count() != 1
            || headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(';').next())
                .map(str::trim)
                != Some("application/json")
        {
            return Err(Failure::Invalid);
        }
        let source = body.map_err(|_| Failure::Invalid)?.to_vec();
        let mut watermark = ctx.clock.now();
        if watermark <= 0 {
            return Err(Failure::Unavailable);
        }
        let _permit = ctx
            .requests
            .clone()
            .try_acquire_owned()
            .map_err(|_| Failure::Unavailable)?;
        // A single constant budget key limits all public reads without tracking visitors.
        ctx.rate
            .lock()
            .map_err(|_| Failure::Unavailable)?
            .session([0; 32], watermark)
            .map_err(rate)?;
        tokio::time::timeout(Duration::from_secs(2), async {
            let input = ctx
                .workers
                .run(move || {
                    let input: DailyRankingsInput =
                        serde_json::from_slice(&source).map_err(|_| ReplayError::Malformed)?;
                    if input.v != 1 {
                        return Err(ReplayError::Malformed);
                    }
                    PublishedDaily::new(
                        &input.date,
                        input.seed_version,
                        liar_core::rules::RulesSnapshot::bundled(),
                    )?;
                    Ok(input)
                })
                .await?;
            observe(ctx.clock.as_ref(), &mut watermark)?;
            let snapshot = ctx.reader.read(&input.date, input.seed_version).await?;
            let now = observe(ctx.clock.as_ref(), &mut watermark)?;
            let encoded = ctx
                .workers
                .run(move || {
                    let reply = match snapshot {
                        Some(snapshot) => snapshot
                            .project(&input.date, input.seed_version, now)
                            .map_err(|_| ReplayError::Unavailable)?,
                        None => empty_rankings(),
                    };
                    let bytes = serde_json::to_vec(&reply).map_err(|_| ReplayError::Unavailable)?;
                    if bytes.len() > 65536 {
                        return Err(ReplayError::Unavailable);
                    }
                    Ok(bytes)
                })
                .await?;
            observe(ctx.clock.as_ref(), &mut watermark)?;
            Ok(([(header::CONTENT_TYPE, "application/json")], encoded).into_response())
        })
        .await
        .map_err(|_| Failure::Unavailable)?
    }
    .await;
    result.unwrap_or_else(failure)
}
