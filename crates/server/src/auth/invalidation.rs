//! Memory authority is invalidated before persistent revocation commits.
pub type InvalidationBarrier = Box<dyn Send>;
pub trait SessionInvalidator: Send + Sync {
    fn session(&self, hash: [u8; 32]) -> InvalidationBarrier;
    fn account(&self, account: uuid::Uuid) -> InvalidationBarrier;
}
pub struct NoSessionInvalidator;
impl SessionInvalidator for NoSessionInvalidator {
    fn session(&self, _: [u8; 32]) -> InvalidationBarrier {
        Box::new(())
    }
    fn account(&self, _: uuid::Uuid) -> InvalidationBarrier {
        Box::new(())
    }
}
