//! Memory authority is invalidated before persistent revocation commits.
pub type InvalidationBarrier = Box<dyn Send>;
pub trait SessionInvalidator: Send + Sync {
    fn session(&self, hash: [u8; 32]) -> InvalidationBarrier;
    fn account(&self, account: uuid::Uuid) -> InvalidationBarrier;
}
pub struct CombinedSessionInvalidator {
    first: std::sync::Arc<dyn SessionInvalidator>,
    second: std::sync::Arc<dyn SessionInvalidator>,
}
impl CombinedSessionInvalidator {
    pub fn new(
        first: std::sync::Arc<dyn SessionInvalidator>,
        second: std::sync::Arc<dyn SessionInvalidator>,
    ) -> Self {
        Self { first, second }
    }
}
impl SessionInvalidator for CombinedSessionInvalidator {
    fn session(&self, hash: [u8; 32]) -> InvalidationBarrier {
        Box::new((self.first.session(hash), self.second.session(hash)))
    }
    fn account(&self, account: uuid::Uuid) -> InvalidationBarrier {
        Box::new((self.first.account(account), self.second.account(account)))
    }
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
