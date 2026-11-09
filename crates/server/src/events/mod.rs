//! Typed product facts. Collection, current authority and persistence are separate ports.
mod aggregate_maintenance;
mod aggregate_retention;
mod aggregates;
mod http;
mod maintenance;
mod metric_reader;
mod metrics;
mod model;
mod optional_model;
mod optional_postgres;
mod optional_records;
mod optional_retention;
mod pg_aggregates;
mod pg_metrics;
mod pg_records;
mod pg_source;
mod policy;
mod postgres;
mod producers;
mod retention;
mod source_retention;
pub use aggregate_maintenance::*;
pub use aggregate_retention::*;
pub use aggregates::*;
pub use http::*;
pub use maintenance::*;
pub use metric_reader::*;
pub use metrics::*;
pub use model::*;
pub use optional_model::*;
pub use optional_postgres::*;
pub use optional_retention::*;
pub use pg_aggregates::*;
pub use pg_metrics::*;
pub use policy::*;
pub use postgres::*;
pub(crate) use producers::{collect_daily, collect_online};
pub use retention::*;
pub use source_retention::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventError {
    Invalid,
    Unsupported,
    Expired,
    Consent,
    Conflict,
    Capacity,
    Incomplete,
}
