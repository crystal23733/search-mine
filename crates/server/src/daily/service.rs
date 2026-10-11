use super::{boundary::*, *};
use liar_core::rules::RulesSnapshot;
use liar_protocol::daily::*;
use serde::Serialize;
use std::sync::Arc;

pub(super) struct DailyService {
    pub repository: Arc<dyn DailyRepository>,
    pub verifier: BoundedReplayVerifier,
    pub clock: Arc<dyn crate::auth::AuthClock>,
}
fn bytes(value: &impl Serialize) -> Result<Vec<u8>, Failure> {
    let bytes = serde_json::to_vec(value).map_err(|_| Failure::Unavailable)?;
    if bytes.len() > 65536 {
        return Err(Failure::Unavailable);
    }
    Ok(bytes)
}
impl DailyService {
    async fn definition(
        &self,
        date: &str,
        version: u16,
        watermark: &mut i64,
    ) -> Result<Arc<PublishedDaily>, Failure> {
        let stored = self.repository.definition(date, version).await?;
        observe(self.clock.as_ref(), watermark)?;
        let accepted = match stored {
            Some(value) => value,
            None => {
                self.repository
                    .publish(PublishedDaily::new(
                        date,
                        version,
                        RulesSnapshot::bundled(),
                    )?)
                    .await?
            }
        };
        observe(self.clock.as_ref(), watermark)?;
        let checked = PublishedDaily::new(date, version, accepted.rules().clone())
            .map_err(|_| Failure::Unavailable)?;
        if checked.metadata() != accepted.metadata() {
            return Err(Failure::Unavailable);
        }
        Ok(Arc::new(accepted))
    }
    pub async fn execute(
        &self,
        session: DailySession,
        input: &Input,
        watermark: &mut i64,
    ) -> Result<Reply, Failure> {
        match input {
            Input::Definition(date, version) => Ok(Reply::Definition(
                self.definition(date, *version, watermark).await?,
            )),
            Input::Submit(attempt, replay, source) => {
                let definition = self
                    .definition(
                        &replay.metadata.date,
                        replay.metadata.seed_version,
                        watermark,
                    )
                    .await?;
                let evidence = self.verifier.verify(definition.clone(), source).await?;
                observe(self.clock.as_ref(), watermark)?;
                let saved = self.repository.submit(session, *attempt, &evidence).await?;
                observe(self.clock.as_ref(), watermark)?;
                Ok(Reply::Submit {
                    definition,
                    saved,
                    evidence,
                })
            }
            Input::Own(metadata) => {
                let definition = self
                    .definition(&metadata.date, metadata.seed_version, watermark)
                    .await?;
                if definition.metadata() != metadata {
                    return Err(Failure::Invalid);
                }
                let saved = self.repository.own(session, metadata).await?;
                observe(self.clock.as_ref(), watermark)?;
                Ok(Reply::Own { definition, saved })
            }
        }
    }
}
pub(super) enum Reply {
    Definition(Arc<PublishedDaily>),
    Submit {
        definition: Arc<PublishedDaily>,
        saved: DailySubmission,
        evidence: ReplayEvidence,
    },
    Own {
        definition: Arc<PublishedDaily>,
        saved: Option<DailyReceipt>,
    },
}
fn trust(receipt: Option<&DailyReceipt>) -> DailyTrust {
    if receipt.is_some_and(|receipt| receipt.verified_elapsed_ms.is_some()) {
        DailyTrust::VerifiedTime
    } else {
        DailyTrust::VerifiedCompletion
    }
}
impl Reply {
    pub fn encode(self, expected: &Expected, now: i64) -> Result<Vec<u8>, Failure> {
        match (self, &expected.input) {
            (Self::Definition(definition), Input::Definition(_, _)) => bytes(&DailyDefinition {
                v: 1,
                metadata: definition.metadata().clone(),
                rules: definition.rules().clone(),
            }),
            (
                Self::Submit {
                    definition,
                    saved,
                    evidence,
                },
                Input::Submit(attempt, _, _),
            ) => {
                let (disposition, receipt) = match saved {
                    DailySubmission::Official(receipt) => {
                        if receipt.attempt != *attempt
                            || receipt.metadata != *evidence.metadata()
                            || receipt.opened_safe as usize != evidence.opened_safe()
                            || receipt.mistakes != evidence.mistakes()
                            || receipt.replayed_elapsed_ms != evidence.replayed_elapsed_ms()
                        {
                            return Err(Failure::Unavailable);
                        }
                        (DailyDisposition::Official, receipt)
                    }
                    DailySubmission::Practice(receipt) => {
                        if receipt.attempt == *attempt {
                            return Err(Failure::Unavailable);
                        }
                        (DailyDisposition::Practice, receipt)
                    }
                };
                bytes(&DailySubmitResponse {
                    v: 1,
                    account_id: expected.account.to_string(),
                    attempt_id: attempt.to_string(),
                    disposition,
                    trust: trust(Some(&receipt)),
                    receipt: completion(&receipt, &definition, now)?,
                })
            }
            (Self::Own { definition, saved }, Input::Own(_)) => bytes(&DailyOwnResponse {
                v: 1,
                account_id: expected.account.to_string(),
                trust: trust(saved.as_ref()),
                receipt: saved
                    .map(|r| completion(&r, &definition, now))
                    .transpose()?,
            }),
            _ => Err(Failure::Unavailable),
        }
    }
}
