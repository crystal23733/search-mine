//! Loopback-only browser fixture. Production never selects these provider proofs or clock controls.
use axum::{
    Json, Router,
    routing::{get, post},
};
use liar_core::{
    board::{Cell, CellId},
    generator::{BoardGenerator, GenerationBudget},
    rules::RulesSnapshot,
};
use liar_server::auth::*;
use liar_server::{lobby::*, online::*};
use sqlx::postgres::PgPoolOptions;
use std::sync::{
    Arc,
    atomic::{AtomicI64, AtomicU64, Ordering},
};
use zeroize::Zeroizing;

const ORIGIN: &str = "https://localhost:8443";
struct Clock {
    auth: AtomicI64,
    game: AtomicU64,
}
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.auth.load(Ordering::SeqCst)
    }
}
impl MatchClock for Clock {
    fn now_ms(&self) -> u64 {
        self.game.load(Ordering::SeqCst)
    }
}
struct Seed(u64);
impl SeedSource for Seed {
    fn seed(&self) -> Result<[u8; 8], liar_protocol::online::OnlineError> {
        Ok(self.0.to_le_bytes())
    }
}
struct Providers;
impl OAuthProvider for Providers {
    fn available(&self, _: Provider) -> bool {
        true
    }
    fn authorize(&self, provider: Provider, auth: &Authorization) -> Result<String, AuthError> {
        let endpoint = match provider {
            Provider::Google => "https://accounts.google.com/o/oauth2/v2/auth",
            Provider::Apple => "https://appleid.apple.com/auth/authorize",
            Provider::Kakao => "https://kauth.kakao.com/oauth/authorize",
            Provider::Naver => "https://nid.naver.com/oauth2.0/authorize",
        };
        let mut url = url::Url::parse(endpoint).map_err(|_| AuthError::Invalid)?;
        url.query_pairs_mut()
            .append_pair("state", &auth.state.expose())
            .append_pair("nonce", &auth.nonce.expose())
            .append_pair(
                "redirect_uri",
                &format!("{ORIGIN}/api/v1/auth/{}/callback", provider.as_str()),
            );
        Ok(url.into())
    }
    async fn exchange(
        &self,
        transaction: &AuthTransaction,
        _: &SecretToken,
        code: &str,
        _: Option<&[u8]>,
        _: i64,
    ) -> Result<VerifiedIdentity, AuthError> {
        let subject = code.strip_prefix("fixture-").ok_or(AuthError::Invalid)?;
        uuid::Uuid::parse_str(subject).map_err(|_| AuthError::Invalid)?;
        Ok(VerifiedIdentity {
            subject: Zeroizing::new(subject.to_owned()),
            apple_refresh: (transaction.provider == Provider::Apple)
                .then(|| Zeroizing::new(format!("fixture-refresh-{subject}"))),
        })
    }
}
impl AppleProvider for Providers {
    async fn notification(&self, _: &str, _: i64) -> Result<AppleNotification, AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn revoke_apple(&self, _: &str, _: i64) -> Result<(), AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn check_apple(&self, _: &str, _: i64) -> Result<AppleCredentialStatus, AuthError> {
        Err(AuthError::Unavailable)
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Advance {
    seconds: u16,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    // Migrations run before this process. The serving fixture uses the production adapters.
    let vault = Arc::new(AeadVault::new(1, vec![(1, [46; 32])]).expect("test vault"));
    let sockets = AuthorityRegistry::new(32).map_err(|_| "fixture sockets")?;
    let authorities = AuthorityRegistry::new(32).map_err(|_| "fixture lobby authorities")?;
    let store = PgAuthStore::with_vault(pool.clone(), vault.clone()).with_invalidations(Arc::new(
        CombinedSessionInvalidator::new(sockets.clone(), authorities.clone()),
    ));
    let service = AuthService::new(
        store.clone(),
        vault,
        DigestKeys::new(1, vec![(1, [47; 32])]).expect("test digests"),
    );
    let clock = Arc::new(Clock {
        auth: AtomicI64::new(1_800_000_000),
        game: AtomicU64::new(0),
    });
    let security = Arc::new(BrowserSecurity::new(ORIGIN, [48; 32]).expect("loopback origin"));
    let registry = MatchRegistry::new(
        MatchLimits {
            matches: 16,
            mailbox: 32,
            outgoing: 64,
            proof_workers: 1,
        },
        clock.clone(),
        clock.clone(),
        sockets,
        Arc::new(PgResultRepository::new(pool, clock.clone())),
    )
    .map_err(|_| "fixture registry")?;
    let sessions = Arc::new(PgSessionReader::new(store, clock.clone()));
    // A small certified board makes browser inputs deterministic; production keeps bundled rules.
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    rules.gauge_capacity = 1;
    let rules = RulesSnapshot::from_rules(rules).map_err(|_| "fixture rules")?;
    let seed = (0..64)
        .find(|&seed| {
            BoardGenerator::generate(rules.rules.board_spec(), seed, GenerationBudget::default())
                .is_ok_and(|g| {
                    g.board.cell(CellId(5)) == Some(Cell::Mine)
                        && g.board.cell(CellId(7)) == Some(Cell::Mine)
                })
        })
        .ok_or("fixture seed")?;
    let lobby = LobbyService::new(
        LobbyLimits {
            capacity: 32,
            workers: 2,
        },
        Arc::new(
            BoardPool::start(rules, 4, 1, Arc::new(Seed(seed)))
                .map_err(|_| "fixture board pool")?,
        ),
        Arc::new(CoreMatchPreparer),
        Arc::new(OsRoomCodeSource),
        registry.clone(),
        BotExecutor::new(2, Arc::new(CoreBotFactory)).map_err(|_| "fixture bots")?,
        LobbyAuthentication {
            authorities,
            clock: clock.clone(),
        },
    )
    .map_err(|_| "fixture lobby")?;
    let advance_clock = clock.clone();
    let router = account_auth_router(service, Providers, security.clone(), clock.clone())
        .merge(
            lobby_router(lobby, sessions.clone(), security, 16)
                .map_err(|_| "fixture lobby HTTP")?,
        )
        .merge(websocket_router(ORIGIN, sessions, registry, 32).map_err(|_| "fixture WebSocket")?)
        .merge(
            Router::new()
                .route("/__fixture/ready", get(|| async { "test-only" }))
                .route(
                    "/__fixture/advance",
                    post(move |Json(body): Json<Advance>| {
                        let clock = advance_clock.clone();
                        async move {
                            clock
                                .auth
                                .fetch_add(i64::from(body.seconds.min(3600)), Ordering::SeqCst);
                            clock.game.fetch_add(
                                u64::from(body.seconds.min(3600)) * 1000,
                                Ordering::SeqCst,
                            );
                            "advanced"
                        }
                    }),
                ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001").await?;
    axum::serve(listener, router).await?;
    Ok(())
}
