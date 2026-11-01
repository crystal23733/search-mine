//! Current session consent; no public HTTP or collection adapter is enabled here.
mod model;
mod postgres;
pub use model::*;
pub use postgres::*;
