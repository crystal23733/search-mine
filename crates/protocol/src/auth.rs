use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Serialize, TS)]
pub struct AuthAccount {
    pub id: String,
    pub nickname: Option<String>,
}
#[derive(Debug, Serialize, TS)]
pub struct AuthProviderStatus {
    pub provider: String,
    pub available: bool,
}
#[derive(Debug, Serialize, TS)]
pub struct AuthBootstrap {
    pub providers: Vec<AuthProviderStatus>,
    pub account: Option<AuthAccount>,
    pub session_revision: Option<String>,
    pub csrf: Option<String>,
}
#[derive(Debug, Serialize, TS)]
pub struct AuthStart {
    pub authorize_url: String,
}
#[derive(Debug, Serialize, TS)]
pub struct AuthFailure {
    pub code: String,
}
pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        AuthAccount::decl(config),
        AuthProviderStatus::decl(config),
        AuthBootstrap::decl(config),
        AuthStart::decl(config),
        AuthFailure::decl(config),
        AuthIdentity::decl(config),
        AuthExport::decl(config),
        AuthErasure::decl(config),
    ]
}
#[derive(Debug, Serialize, TS)]
pub struct AuthIdentity {
    pub provider: String,
    #[ts(type = "number")]
    pub linked_at: i64,
}
#[derive(Debug, Serialize, TS)]
pub struct AuthExport {
    pub account: AuthAccount,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub last_seen_at: i64,
    pub identities: Vec<AuthIdentity>,
}
#[derive(Debug, Serialize, TS)]
pub struct AuthErasure {
    pub manual_apple_disconnect: bool,
}
