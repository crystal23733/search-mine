mod model;
pub use model::{Ledger, MAX_BYTES, Snapshot, SnapshotInspection};
mod postgres;
pub use postgres::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryError {
    Invalid,
    Blocked,
    Busy,
    Unavailable,
}
