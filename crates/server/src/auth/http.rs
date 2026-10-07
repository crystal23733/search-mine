use super::browser::cookie;
use super::*;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, RawQuery, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use liar_protocol::auth::*;
use std::sync::Arc;
const PROVIDERS: [Provider; 4] = [
    Provider::Google,
    Provider::Apple,
    Provider::Kakao,
    Provider::Naver,
];
type RateBuckets = std::collections::HashMap<([u8; 32], bool), (i64, u8)>;
struct Context<S, V, D, P> {
    service: AuthService<S, V, D>,
    providers: P,
    security: BrowserSecurity,
    clock: Arc<dyn AuthClock>,
    rate: std::sync::Mutex<RateBuckets>,
}
impl<S, V, D, P> Context<S, V, D, P> {
    fn permit(&self, browser: &SecretToken, callback: bool, now: i64) -> Result<(), AuthError> {
        if now < 0 {
            return Err(AuthError::Invalid);
        }
        let mut rate = self.rate.lock().map_err(|_| AuthError::Unavailable)?;
        rate.retain(|_, (created, _)| now >= *created && now - *created < 300);
        let key = (browser.hash(), callback);
        if rate.len() >= 4096 && !rate.contains_key(&key) {
            return Err(AuthError::Unavailable);
        }
        let entry = rate.entry(key).or_insert((now, 0));
        let max = if callback { 20 } else { 5 };
        if entry.1 >= max {
            return Err(AuthError::Unavailable);
        }
        entry.1 += 1;
        Ok(())
    }
}
pub fn auth_router<
    S: AuthStore + 'static,
    V: CredentialVault + 'static,
    D: SubjectDigester + 'static,
    P: OAuthProvider + 'static,
>(
    service: AuthService<S, V, D>,
    providers: P,
    security: BrowserSecurity,
    clock: Arc<dyn AuthClock>,
) -> Router {
    let context = Arc::new(Context {
        service,
        providers,
        security,
        clock,
        rate: Default::default(),
    });
    Router::new()
        .route("/api/v1/auth/bootstrap", get(bootstrap::<S, V, D, P>))
        .route("/api/v1/auth/providers", get(provider_status::<S, V, D, P>))
        .route("/api/v1/auth/{provider}/start", post(start::<S, V, D, P>))
        .route(
            "/api/v1/auth/{provider}/callback",
            get(callback::<S, V, D, P>),
        )
        .route(
            "/api/v1/me",
            get(me::<S, V, D, P>).patch(nickname::<S, V, D, P>),
        )
        .route("/api/v1/auth/logout", post(logout::<S, V, D, P>))
        .layer(DefaultBodyLimit::max(4096))
        .layer(middleware::map_response(no_store))
        .with_state(context)
}
async fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}
pub fn disabled_auth_router() -> Router {
    async fn bootstrap() -> Json<AuthBootstrap> {
        Json(AuthBootstrap {
            providers: PROVIDERS
                .into_iter()
                .map(|p| AuthProviderStatus {
                    provider: p.as_str().into(),
                    available: false,
                })
                .collect(),
            account: None,
            session_revision: None,
            csrf: None,
        })
    }
    async fn providers() -> Json<Vec<AuthProviderStatus>> {
        Json(
            PROVIDERS
                .into_iter()
                .map(|p| AuthProviderStatus {
                    provider: p.as_str().into(),
                    available: false,
                })
                .collect(),
        )
    }
    async fn unavailable() -> Response {
        failure(AuthError::Unavailable)
    }
    Router::new()
        .route("/api/v1/auth/bootstrap", get(bootstrap))
        .route("/api/v1/auth/providers", get(providers))
        .route("/api/v1/auth/{provider}/start", post(unavailable))
        .route("/api/v1/auth/{provider}/callback", get(unavailable))
        .route("/api/v1/me", get(unavailable).patch(unavailable))
        .route("/api/v1/auth/logout", post(unavailable))
        .layer(middleware::map_response(no_store))
}
fn failure(error: AuthError) -> Response {
    let (status, code) = match error {
        AuthError::Unauthenticated => (StatusCode::UNAUTHORIZED, "auth_required"),
        AuthError::Invalid => (StatusCode::BAD_REQUEST, "auth_invalid"),
        AuthError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "auth_unavailable"),
        AuthError::Conflict => (StatusCode::CONFLICT, "auth_conflict"),
    };
    (status, Json(AuthFailure { code: code.into() })).into_response()
}
fn account(value: &Account) -> AuthAccount {
    AuthAccount {
        id: value.id.to_string(),
        nickname: value.nickname.as_ref().map(|n| n.as_str().into()),
    }
}
fn statuses<P: OAuthProvider>(providers: &P) -> Vec<AuthProviderStatus> {
    PROVIDERS
        .into_iter()
        .map(|p| AuthProviderStatus {
            provider: p.as_str().into(),
            available: providers.available(p),
        })
        .collect()
}
fn set_cookie(response: &mut Response, value: String) -> Result<(), AuthError> {
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&value).map_err(|_| AuthError::Unavailable)?,
    );
    Ok(())
}
async fn bootstrap<S: AuthStore, V: CredentialVault, D: SubjectDigester, P: OAuthProvider>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
    headers: HeaderMap,
) -> Response {
    let result = async {
        let mut cookies = BrowserCookies::parse(&headers)?;
        let now = ctx.clock.now();
        let session = if let Some(token) = &cookies.session {
            ctx.service.session(token, now).await?
        } else {
            None
        };
        let clear = cookies.session.is_some() && session.is_none();
        if clear {
            cookies.session = None;
        }
        let browser = cookies
            .browser
            .take()
            .map(Ok)
            .unwrap_or_else(SecretToken::generate)?;
        let csrf = ctx.security.csrf(&browser, cookies.session.as_ref(), now)?;
        let mut response = Json(AuthBootstrap {
            providers: statuses(&ctx.providers),
            account: session.as_ref().map(|s| account(&s.account)),
            session_revision: session.map(|s| s.id.to_string()),
            csrf: Some(csrf),
        })
        .into_response();
        set_cookie(&mut response, cookie(BROWSER_COOKIE, Some(&browser), 300))?;
        if clear {
            set_cookie(&mut response, cookie(SESSION_COOKIE, None, 0))?;
        }
        Ok::<_, AuthError>(response)
    }
    .await;
    result.unwrap_or_else(failure)
}
async fn provider_status<S: AuthStore, V: CredentialVault, D: SubjectDigester, P: OAuthProvider>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
) -> Json<Vec<AuthProviderStatus>> {
    Json(statuses(&ctx.providers))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StartBody {
    locale: String,
    return_path: String,
}
async fn start<S: AuthStore, V: CredentialVault, D: SubjectDigester, P: OAuthProvider>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Result<Json<StartBody>, JsonRejection>,
) -> Response {
    let result = async {
        let cookies = BrowserCookies::parse(&headers)?;
        let now = ctx.clock.now();
        ctx.security.require(&headers, &cookies, now)?;
        let Json(body) = body.map_err(|_| AuthError::Invalid)?;
        let provider = Provider::parse(&provider)?;
        let locale = AuthLocale::parse(&body.locale)?;
        let return_path = ReturnPath::parse(&body.return_path)?;
        if !ctx.providers.available(provider) {
            return Err(AuthError::Unavailable);
        }
        let browser = cookies.browser.as_ref().ok_or(AuthError::Invalid)?;
        ctx.permit(browser, false, now)?;
        let auth = ctx
            .service
            .start_localized(
                browser,
                provider,
                AuthIntent::Login,
                return_path,
                locale,
                now,
            )
            .await?;
        Ok::<_, AuthError>(
            Json(AuthStart {
                authorize_url: ctx.providers.authorize(provider, &auth)?,
            })
            .into_response(),
        )
    }
    .await;
    result.unwrap_or_else(failure)
}
fn redirect(path: String) -> Result<Response, AuthError> {
    let mut response = StatusCode::SEE_OTHER.into_response();
    response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&path).map_err(|_| AuthError::Invalid)?,
    );
    Ok(response)
}
async fn callback<S: AuthStore, V: CredentialVault, D: SubjectDigester, P: OAuthProvider>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
    Path(provider): Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Response {
    let result = async {
        let query = query
            .filter(|q| q.len() <= 4096)
            .ok_or(AuthError::Invalid)?;
        let mut pairs = std::collections::HashMap::new();
        for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
            if !matches!(k.as_ref(), "state" | "code" | "error" | "error_description")
                || pairs.insert(k.into_owned(), v.into_owned()).is_some()
            {
                return Err(AuthError::Invalid);
            }
        }
        let state = SecretToken::parse(pairs.get("state").ok_or(AuthError::Invalid)?)?;
        let cookies = BrowserCookies::parse(&headers)?;
        let browser = cookies.browser.as_ref().ok_or(AuthError::Invalid)?;
        let provider = Provider::parse(&provider)?;
        let now = ctx.clock.now();
        ctx.permit(browser, true, now)?;
        let tx = ctx.service.consume(&state, browser, provider, now).await?;
        let locale = tx.locale.as_str();
        let destination = tx.return_path.as_str();
        let complete = async {
            if pairs.contains_key("error") {
                return Err(AuthError::Invalid);
            }
            let code = pairs.get("code").ok_or(AuthError::Invalid)?;
            let verifier = ctx.service.verifier(&tx)?;
            let identity = ctx
                .providers
                .exchange(
                    &tx,
                    &state,
                    code,
                    verifier.as_ref().map(|v| v.as_slice()),
                    now,
                )
                .await?;
            let issued = ctx
                .service
                .finish_login(
                    tx,
                    provider,
                    &identity.subject,
                    cookies.session.as_ref(),
                    ctx.clock.now(),
                )
                .await?;
            let path = if issued.account.nickname.is_none() {
                format!("/{locale}/onboarding?return_path={destination}")
            } else {
                return_url(locale, destination)
            };
            let mut response = redirect(path)?;
            set_cookie(
                &mut response,
                cookie(SESSION_COOKIE, Some(&issued.token), SESSION_SECONDS),
            )?;
            Ok::<_, AuthError>(response)
        }
        .await;
        complete.or_else(|_| redirect(format!("/{locale}/login?error=auth_failed")))
    }
    .await;
    result.unwrap_or_else(failure)
}
fn return_url(locale: &str, destination: &str) -> String {
    match destination {
        "home" => format!("/{locale}/"),
        _ => format!("/{locale}/{destination}"),
    }
}
async fn me<S: AuthStore, V: CredentialVault, D: SubjectDigester, P: OAuthProvider>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
    headers: HeaderMap,
) -> Response {
    let result = async {
        let cookies = BrowserCookies::parse(&headers)?;
        let token = cookies.session.ok_or(AuthError::Unauthenticated)?;
        let session = ctx
            .service
            .session(&token, ctx.clock.now())
            .await?
            .ok_or(AuthError::Unauthenticated)?;
        Ok::<_, AuthError>(Json(account(&session.account)).into_response())
    }
    .await;
    result.unwrap_or_else(failure)
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NicknameBody {
    nickname: String,
}
async fn nickname<S: AuthStore, V: CredentialVault, D: SubjectDigester, P: OAuthProvider>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
    headers: HeaderMap,
    body: Result<Json<NicknameBody>, JsonRejection>,
) -> Response {
    let result = async {
        let cookies = BrowserCookies::parse(&headers)?;
        let now = ctx.clock.now();
        ctx.security.require(&headers, &cookies, now)?;
        let token = cookies.session.as_ref().ok_or(AuthError::Invalid)?;
        let session = ctx
            .service
            .session(token, now)
            .await?
            .ok_or(AuthError::Invalid)?;
        let Json(body) = body.map_err(|_| AuthError::Invalid)?;
        let nickname = Nickname::parse(&body.nickname)?;
        if !ctx
            .service
            .store
            .nickname(session.account.id, nickname)
            .await?
        {
            return Err(AuthError::Invalid);
        }
        let session = ctx
            .service
            .session(token, ctx.clock.now())
            .await?
            .ok_or(AuthError::Invalid)?;
        Ok::<_, AuthError>(Json(account(&session.account)).into_response())
    }
    .await;
    result.unwrap_or_else(failure)
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyBody {}
async fn logout<S: AuthStore, V: CredentialVault, D: SubjectDigester, P: OAuthProvider>(
    State(ctx): State<Arc<Context<S, V, D, P>>>,
    headers: HeaderMap,
    body: Result<Json<EmptyBody>, JsonRejection>,
) -> Response {
    let result = async {
        let cookies = BrowserCookies::parse(&headers)?;
        ctx.security.require(&headers, &cookies, ctx.clock.now())?;
        let _ = body.map_err(|_| AuthError::Invalid)?;
        let token = cookies.session.as_ref().ok_or(AuthError::Invalid)?;
        ctx.service.store.logout(token.hash()).await?;
        let mut response = StatusCode::NO_CONTENT.into_response();
        set_cookie(&mut response, cookie(SESSION_COOKIE, None, 0))?;
        Ok::<_, AuthError>(response)
    }
    .await;
    result.unwrap_or_else(failure)
}
