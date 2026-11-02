//! Typed product facts. Collection, current authority and persistence are separate ports.
mod aggregate_maintenance;
mod aggregate_retention;
mod aggregates;
mod maintenance;
mod metric_reader;
mod metrics;
mod model;
mod pg_aggregates;
mod pg_metrics;
mod pg_records;
mod pg_source;
mod policy;
mod postgres;
mod producers;
mod retention;
pub use aggregate_maintenance::*;
pub use aggregate_retention::*;
pub use aggregates::*;
pub use maintenance::*;
pub use metric_reader::*;
pub use metrics::*;
pub use model::*;
pub use pg_aggregates::*;
pub use pg_metrics::*;
pub use policy::*;
pub use postgres::*;
pub(crate) use producers::{collect_daily, collect_online};
pub use retention::*;

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
