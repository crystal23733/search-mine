//! Current-session consent control; collection still requires at-write authorization.
mod http;
mod model;
mod postgres;
pub(crate) mod view;
pub use http::*;
pub use model::*;
pub use postgres::*;
