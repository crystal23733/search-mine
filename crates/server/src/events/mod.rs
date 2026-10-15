//! Typed product facts. Collection, current authority and persistence are separate ports.
mod metrics;
mod model;
mod pg_records;
mod pg_source;
mod policy;
mod postgres;
pub use metrics::*;
pub use model::*;
pub(crate) use pg_records::export_events;
pub use policy::*;
pub use postgres::*;

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
