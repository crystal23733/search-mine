use super::{boundary::*, *};
use crate::online::ConnectionAuthority;
use liar_core::rules::RulesSnapshot;
use liar_protocol::daily::*;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

pub(super) struct DailyService {
    pub repository: Arc<dyn DailyRepository>,
    pub verifier: BoundedReplayVerifier,
    pub clock: Arc<dyn crate::auth::AuthClock>,
    pub tickets: Arc<DailyTicketRegistry>,
}
fn today(now: i64) -> Result<String, Failure> {
    chrono::DateTime::from_timestamp(now, 0)
        .map(|time| time.date_naive().to_string())
        .filter(|date| date.len() == 10)
        .ok_or(Failure::Unavailable)
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
        lease: &ConnectionAuthority,
        revision: Uuid,
        input: &Input,
        watermark: &mut i64,
    ) -> Result<Reply, Failure> {
        match input {
            Input::Start(attempt) => {
                let date = today(observe(self.clock.as_ref(), watermark)?)?;
                let definition = self
                    .definition(&date, liar_core::daily::DAILY_SEED_VERSION, watermark)
                    .await?;
                let now = observe(self.clock.as_ref(), watermark)?;
                if today(now)? != date {
                    return Err(Failure::Conflict);
                }
                let grant = self
                    .tickets
                    .start(lease, revision, *attempt, &definition, now)?;
                Ok(Reply::Start { definition, grant })
            }
            Input::Heartbeat(ticket, sequence) => {
                let now = observe(self.clock.as_ref(), watermark)?;
                Ok(Reply::Heartbeat(
                    self.tickets
                        .heartbeat(lease, revision, *ticket, *sequence, now)?,
                ))
            }
            Input::Definition(date, version) => Ok(Reply::Definition(
                self.definition(date, *version, watermark).await?,
            )),
            Input::Submit(attempt, replay, source)
            | Input::TimedSubmit(attempt, _, replay, source) => {
                let definition = self
                    .definition(
                        &replay.metadata.date,
                        replay.metadata.seed_version,
                        watermark,
                    )
                    .await?;
                let evidence = self.verifier.verify(definition.clone(), source).await?;
                let now = observe(self.clock.as_ref(), watermark)?;
                let time = match input {
                    Input::TimedSubmit(_, Some(ticket), _, _) => self
                        .tickets
                        .finish(lease, revision, *ticket, *attempt, &evidence, now)?,
                    _ => None,
                };
                let saved = match time {
                    Some(time) => {
                        self.repository
                            .submit_timed(session, *attempt, &evidence, &time)
                            .await?
                    }
                    None => self.repository.submit(session, *attempt, &evidence).await?,
                };
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
    Start {
        definition: Arc<PublishedDaily>,
        grant: TicketGrant,
    },
    Heartbeat(Option<HeartbeatAck>),
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
            (Self::Start { definition, grant }, Input::Start(attempt)) => {
                bytes(&DailyStartResponse {
                    v: 1,
                    account_id: expected.account.to_string(),
                    attempt_id: attempt.to_string(),
                    ticket_id: grant.id.to_string(),
                    metadata: definition.metadata().clone(),
                    rules: definition.rules().clone(),
                    ttl_ms: grant.ttl_ms,
                    heartbeat_interval_ms: grant.heartbeat_interval_ms,
                })
            }
            (Self::Heartbeat(ack), Input::Heartbeat(ticket, _)) => bytes(&DailyHeartbeatResponse {
                v: 1,
                account_id: expected.account.to_string(),
                ticket_id: ticket.to_string(),
                ack: ack.map(|ack| DailyHeartbeatAck {
                    sequence: ack.sequence,
                    elapsed_ms: ack.elapsed_ms,
                }),
            }),
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
                Input::Submit(attempt, _, _) | Input::TimedSubmit(attempt, _, _, _),
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
