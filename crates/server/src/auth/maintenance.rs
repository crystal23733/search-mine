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
        if !self.providers.available(Provider::Apple) {
            return Ok(());
        }
        for _ in 0..8 {
            let Some(job) = self.store.claim_revoke(self.clock.now()).await? else {
                break;
            };
            let complete = async {
                let bytes = self.vault.open(
                    job.identity,
                    Provider::Apple,
                    CredentialPurpose::AppleRevoke,
                    &job.encrypted,
                )?;
                let refresh = std::str::from_utf8(&bytes).map_err(|_| AuthError::Invalid)?;
                tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    self.providers.revoke_apple(refresh, self.clock.now()),
                )
                .await
                .map_err(|_| AuthError::Unavailable)?
            }
            .await
            .is_ok();
            self.store
                .finish_revoke(&job, complete, self.clock.now())
                .await?;
        }
        for _ in 0..8 {
            let Some(job) = self.store.claim_credential(self.clock.now()).await? else {
                break;
            };
            let outcome = async {
                let bytes = self.vault.open(
                    job.identity,
                    Provider::Apple,
                    CredentialPurpose::AppleRevoke,
                    &job.encrypted,
                )?;
                let refresh = std::str::from_utf8(&bytes).map_err(|_| AuthError::Invalid)?;
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(20),
                    self.providers.check_apple(refresh, self.clock.now()),
                )
                .await
                .map_err(|_| AuthError::Unavailable)??;
                match result {
                    AppleCredentialStatus::Revoked => Ok::<_, AuthError>(CredentialCheck::Revoked),
                    AppleCredentialStatus::Valid {
                        subject,
                        replacement,
                    } => Ok(CredentialCheck::Valid {
                        digests: self.digests.digest(Provider::Apple, &subject)?,
                        replacement,
                    }),
                }
            }
            .await
            .unwrap_or(CredentialCheck::Unavailable);
            self.store
                .finish_credential(&job, outcome, self.clock.now())
                .await?;
        }
        Ok(())
    }
}
