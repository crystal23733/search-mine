use super::*;
use std::sync::Arc;
pub struct AppleMaintenance<S, V, D, P> {
    store: S,
    vault: V,
    digests: D,
    providers: P,
    clock: Arc<dyn AuthClock>,
}
impl<S: AppleMaintenanceStore, V: CredentialVault, D: SubjectDigester, P: AppleProvider>
    AppleMaintenance<S, V, D, P>
{
    pub fn new(store: S, vault: V, digests: D, providers: P, clock: Arc<dyn AuthClock>) -> Self {
        Self {
            store,
            vault,
            digests,
            providers,
            clock,
        }
    }
    pub async fn run_once(&self) -> Result<(), AuthError> {
        let _ = (&self.store, &self.vault, &self.digests, &self.clock);
        if !self.providers.available(Provider::Apple) {
            return Ok(());
        }
        Err(AuthError::Unavailable)
    }
}
