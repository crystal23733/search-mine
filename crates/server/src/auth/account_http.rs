use super::http::{Context, account, base_routes, failure, no_store, set_cookie};
use super::*;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use liar_protocol::auth::{AuthErasure, AuthExport, AuthIdentity, AuthStart};
use std::sync::Arc;
pub fn account_auth_router<
    S: AppleMaintenanceStore + 'static,
    V: CredentialVault + 'static,
    D: SubjectDigester + 'static,
    P: AppleProvider + 'static,
>(
    service: AuthService<S, V, D>,
    providers: P,
    security: BrowserSecurity,
    clock: Arc<dyn AuthClock>,
) -> Router {
    let ctx = Arc::new(Context {
        service,
        providers,
        security,
        clock,
        rate: Default::default(),
    });
    base_routes()
        .route("/api/v1/me/identities", get(linked::<S, V, D, P>))
        .route(
            "/api/v1/me/identities/{provider}/link",
            post(link::<S, V, D, P>),
        )
        .route("/api/v1/auth/{provider}/reauth", post(reauth::<S, V, D, P>))
        .route(
            "/api/v1/me/identities/{provider}",
            delete(unlink::<S, V, D, P>),
        )
        .route("/api/v1/me/export", post(export::<S, V, D, P>))
        .route("/api/v1/me", delete(erase::<S, V, D, P>))
        .layer(DefaultBodyLimit::max(4096))
        .route(
            "/api/v1/auth/apple/notifications",
            post(notification::<S, V, D, P>).layer(DefaultBodyLimit::max(32768)),
        )
        .layer(middleware::map_response(no_store))
        .with_state(ctx)
}
async fn authority<S: AuthStore, V: CredentialVault, D: SubjectDigester, P>(
    ctx: &Context<S, V, D, P>,
    headers: &HeaderMap,
    mutation: bool,
) -> Result<(BrowserCookies, SessionAuthority), AuthError> {
    let cookies = BrowserCookies::parse(headers)?;
    let now = ctx.clock.now();
    if mutation {
        ctx.security.require(headers, &cookies, now)?;
    }
    let token = cookies.session.as_ref().ok_or(AuthError::Unauthenticated)?;
    let session = ctx
        .service
        .session(token, now)
        .await?
        .ok_or(AuthError::Unauthenticated)?;
    let auth = SessionAuthority {
        account: session.account.id,
        hash: token.hash(),
        now,
    };
    Ok((cookies, auth))
}
fn identity_dto(i: LinkedIdentity) -> AuthIdentity {
    AuthIdentity {
        provider: i.provider.as_str().into(),
        linked_at: i.linked_at,
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StartBody {
    locale: String,
    return_path: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyBody {}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NotificationBody {
    payload: String,
}
async fn linked<S: AccountStore, V: CredentialVault, D: SubjectDigester, P: OAuthProvider>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
    headers: HeaderMap,
) -> Response {
    let result = async {
        let (_, auth) = authority(&ctx, &headers, false).await?;
        let identities = ctx.service.store.identities(auth).await?;
        Ok::<_, AuthError>(
            Json(identities.into_iter().map(identity_dto).collect::<Vec<_>>()).into_response(),
        )
    }
    .await;
    result.unwrap_or_else(failure)
}
async fn account_start<
    S: AccountStore,
    V: CredentialVault,
    D: SubjectDigester,
    P: OAuthProvider,
>(
    ctx: &Context<S, V, D, P>,
    provider: &str,
    headers: &HeaderMap,
    body: Result<Json<StartBody>, JsonRejection>,
    is_link: bool,
) -> Result<Response, AuthError> {
    let (cookies, authority) = authority(ctx, headers, true).await?;
    let Json(body) = body.map_err(|_| AuthError::Invalid)?;
    let provider = Provider::parse(provider)?;
    let request = AccountAuthorization {
        provider,
        intent: if is_link {
            AuthIntent::Link(authority.account)
        } else {
            AuthIntent::Reauth(authority.account)
        },
        locale: AuthLocale::parse(&body.locale)?,
        return_path: ReturnPath::parse(&body.return_path)?,
    };
    if !ctx.providers.available(provider) {
        return Err(AuthError::Unavailable);
    }
    if !is_link
        && !ctx
            .service
            .store
            .identities(authority)
            .await?
            .iter()
            .any(|i| i.provider == provider)
    {
        return Err(AuthError::Invalid);
    }
    let browser = cookies.browser.as_ref().ok_or(AuthError::Invalid)?;
    ctx.permit(browser, false, authority.now)?;
    let authorization = ctx
        .service
        .start_account(
            browser,
            cookies.session.as_ref().ok_or(AuthError::Unauthenticated)?,
            request,
            authority.now,
        )
        .await?;
    Ok(Json(AuthStart {
        authorize_url: ctx.providers.authorize(provider, &authorization)?,
    })
    .into_response())
}
async fn link<S: AccountStore, V: CredentialVault, D: SubjectDigester, P: OAuthProvider>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Result<Json<StartBody>, JsonRejection>,
) -> Response {
    account_start(&ctx, &provider, &headers, body, true)
        .await
        .unwrap_or_else(failure)
}
async fn reauth<S: AccountStore, V: CredentialVault, D: SubjectDigester, P: OAuthProvider>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Result<Json<StartBody>, JsonRejection>,
) -> Response {
    account_start(&ctx, &provider, &headers, body, false)
        .await
        .unwrap_or_else(failure)
}
async fn export<S: AccountStore, V: CredentialVault, D: SubjectDigester, P: OAuthProvider>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
    headers: HeaderMap,
    body: Result<Json<EmptyBody>, JsonRejection>,
) -> Response {
    let result = async {
        let (_, auth) = authority(&ctx, &headers, true).await?;
        let _ = body.map_err(|_| AuthError::Invalid)?;
        let data = ctx.service.store.export(auth).await?;
        Ok::<_, AuthError>(
            Json(AuthExport {
                account: account(&data.account),
                created_at: data.created_at,
                last_seen_at: data.last_seen_at,
                identities: data.identities.into_iter().map(identity_dto).collect(),
            })
            .into_response(),
        )
    }
    .await;
    result.unwrap_or_else(failure)
}
fn erased_response(result: ErasureResult) -> Result<Response, AuthError> {
    let mut response = Json(AuthErasure {
        manual_apple_disconnect: result.manual_apple_disconnect,
    })
    .into_response();
    set_cookie(
        &mut response,
        super::browser::cookie(SESSION_COOKIE, None, 0),
    )?;
    Ok(response)
}
async fn erase<S: AccountStore, V: CredentialVault, D: SubjectDigester, P: OAuthProvider>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
    headers: HeaderMap,
    body: Result<Json<EmptyBody>, JsonRejection>,
) -> Response {
    let result = async {
        let (_, auth) = authority(&ctx, &headers, true).await?;
        let _ = body.map_err(|_| AuthError::Invalid)?;
        erased_response(ctx.service.store.erase_authorized(auth).await?)
    }
    .await;
    result.unwrap_or_else(failure)
}
async fn unlink<S: AccountStore, V: CredentialVault, D: SubjectDigester, P: OAuthProvider>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Result<Json<EmptyBody>, JsonRejection>,
) -> Response {
    let result = async {
        let (_, auth) = authority(&ctx, &headers, true).await?;
        let _ = body.map_err(|_| AuthError::Invalid)?;
        erased_response(
            ctx.service
                .store
                .unlink(auth, Provider::parse(&provider)?)
                .await?,
        )
    }
    .await;
    result.unwrap_or_else(failure)
}
async fn notification<
    S: AppleMaintenanceStore,
    V: CredentialVault,
    D: SubjectDigester,
    P: AppleProvider,
>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
    body: Result<Json<NotificationBody>, JsonRejection>,
) -> Response {
    let result = async {
        let Json(body) = body.map_err(|_| AuthError::Invalid)?;
        let now = ctx.clock.now();
        let event = ctx.providers.notification(&body.payload, now).await?;
        let digests = if event.change == AppleChange::Ignore {
            None
        } else {
            Some(
                ctx.service
                    .subject_digests(Provider::Apple, &event.subject)?,
            )
        };
        ctx.service
            .store
            .apply_notification(AppleNoticeWrite {
                jti_hash: event.jti_hash,
                digests,
                occurred_at: event.occurred_at,
                now,
            })
            .await?;
        Ok::<_, AuthError>(StatusCode::OK.into_response())
    }
    .await;
    result.unwrap_or_else(failure)
}
