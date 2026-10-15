//! Typed product facts. Collection, current authority and persistence are separate ports.
mod metrics;
mod model;
mod policy;
pub use metrics::*;
pub use model::*;
pub use policy::*;

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
