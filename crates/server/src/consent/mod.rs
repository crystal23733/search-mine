//! Current-session consent control; collection still requires at-write authorization.
mod http;
mod model;
mod postgres;
mod tcf;
pub use tcf::GoogleConsentVerifier;
pub(crate) mod view;
pub use http::*;
pub use model::*;
pub use postgres::*;
