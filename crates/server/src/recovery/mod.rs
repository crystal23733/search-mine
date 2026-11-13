mod model;
pub use model::{Ledger, MAX_BYTES, Snapshot, SnapshotInspection, SnapshotMatch};
mod postgres;
mod resume;
pub use postgres::*;
pub use resume::resume_from_source;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryError {
    Invalid,
    Blocked,
    Busy,
    Unavailable,
}
