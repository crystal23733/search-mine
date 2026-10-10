use super::{MatchPreparer, MatchReservation, PreparedMatch};
use crate::online::{ActiveMatch, AdmissionJournal, ConnectionAuthority};
use liar_protocol::online::OnlineError;
use std::{sync::Arc, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, oneshot, watch};
use uuid::Uuid;

pub(super) struct Receipt {
    id: Uuid,
    journal: Arc<dyn AdmissionJournal>,
    _permit: OwnedSemaphorePermit,
    confirmed: bool,
}
impl Receipt {
    pub fn commit(mut self) {
        self.confirmed = true;
    }
    pub async fn discard(mut self) {
        if matches!(
            tokio::time::timeout(Duration::from_secs(2), self.journal.discard(self.id)).await,
            Ok(Ok(()))
        ) {
            self.confirmed = true;
        }
    }
    pub fn discard_later(self) {
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(self.discard());
        }
    }
}
impl Drop for Receipt {
    fn drop(&mut self) {
        if !self.confirmed {
            // A memory actor may already exist. Never guess by deleting its journal.
            self.journal.fail_closed();
        }
    }
}
pub(super) struct PreparedAdmission {
    pub prepared: PreparedMatch,
    pub reservation: MatchReservation,
    pub participants: [Option<ConnectionAuthority>; 2],
    pub id: Uuid,
    pub receipt: Receipt,
}
pub(super) struct Job {
    pub result: oneshot::Receiver<Result<PreparedAdmission, OnlineError>>,
    pub cancel: watch::Sender<bool>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
        self.result.close();
        if let Ok(Ok(ready)) = self.result.try_recv() {
            ready.receipt.discard_later();
        }
    }
}
pub(super) struct AdmissionWork {
    pub boards: Arc<dyn super::BoardSource>,
    pub preparer: Arc<dyn MatchPreparer>,
    pub journal: Arc<dyn AdmissionJournal>,
    pub permit: OwnedSemaphorePermit,
    pub reservation: MatchReservation,
    pub participants: [Option<ConnectionAuthority>; 2],
}
impl AdmissionWork {
    pub async fn run(
        self,
        mut canceled: watch::Receiver<bool>,
        completed: oneshot::Sender<Result<PreparedAdmission, OnlineError>>,
    ) {
        if *canceled.borrow() {
            return;
        }
        let taken = tokio::select! {
            biased;
            _ = canceled.changed() => return,
            result = tokio::time::timeout(Duration::from_secs(2), self.boards.take(Duration::from_secs(2))) => result.unwrap_or(Err(OnlineError::Unavailable)),
        };
        let board = match taken {
            Ok(board) => board,
            Err(error) => {
                let _ = completed.send(Err(error));
                return;
            }
        };
        let permit = self.permit;
        let preparer = self.preparer;
        let cancellation = canceled.clone();
        let prepared = tokio::task::spawn_blocking(move || {
            // Actual CPU work retains capacity even after its caller has left.
            let ready = if *cancellation.borrow() {
                Err(OnlineError::Unavailable)
            } else {
                preparer.prepare(board)
            };
            (ready, permit)
        })
        .await;
        let (prepared, permit) = match prepared {
            Ok((Ok(prepared), permit)) => (prepared, permit),
            Ok((Err(error), _)) => {
                let _ = completed.send(Err(error));
                return;
            }
            Err(_) => {
                let _ = completed.send(Err(OnlineError::Unavailable));
                return;
            }
        };
        if *canceled.borrow() {
            return;
        }
        let id = Uuid::new_v4();
        let active = ActiveMatch {
            id,
            rules_hash: prepared.engine.rules_hash().to_owned(),
            players: self.reservation.players,
        };
        // Once registration starts, normal cancellation must observe its outcome.
        if let Err(error) = self.journal.register(active).await {
            let _ = completed.send(Err(error));
            return;
        }
        let receipt = Receipt {
            id,
            journal: self.journal,
            _permit: permit,
            confirmed: false,
        };
        if *canceled.borrow() {
            receipt.discard().await;
            return;
        }
        let ready = PreparedAdmission {
            prepared,
            reservation: self.reservation,
            participants: self.participants,
            id,
            receipt,
        };
        if let Err(Ok(ready)) = completed.send(Ok(ready)) {
            ready.receipt.discard().await;
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/admission.rs"]
mod tests;
