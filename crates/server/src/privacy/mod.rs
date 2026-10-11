mod config;
mod http;
mod model;
mod postgres;
mod workers;
use crate::online::PortFuture;
pub use config::*;
pub use http::*;
use liar_protocol::privacy::*;
pub(crate) use model::*;
pub use postgres::PgGameExportReader;
use uuid::Uuid;
pub use workers::ExportWorkers;

#[derive(Clone, Copy)]
pub struct ExportSession {
    pub account: Uuid,
    pub revision: Uuid,
    pub hash: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportError {
    Malformed,
    Unauthorized,
    Reauth,
    Unavailable,
    RateLimited,
}
pub trait GameExportReader: Send + Sync {
    fn page(
        &self,
        session: ExportSession,
        stream: ExportStream,
        after: Option<Uuid>,
    ) -> PortFuture<'_, Result<GameExportPage, ExportError>>;
    fn replay(
        &self,
        session: ExportSession,
        completion: Uuid,
    ) -> PortFuture<'_, Result<ReplayExport, ExportError>>;
}
