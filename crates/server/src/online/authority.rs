use crate::auth::{InvalidationBarrier, SessionInvalidator};
use liar_protocol::online::OnlineError;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::watch;
use uuid::Uuid;

pub struct AuthorityRegistry {
    capacity: usize,
    inner: Mutex<Inner>,
    owner: Weak<Self>,
}
struct Inner {
    generation: u64,
    pending: usize,
    active: HashMap<Uuid, Entry>,
}
struct Entry {
    token: Uuid,
    hash: [u8; 32],
    expires: i64,
    closed: watch::Sender<bool>,
}
struct LeaseIdentity {
    account: Uuid,
    token: Uuid,
    owner: Weak<AuthorityRegistry>,
}
#[derive(Clone)]
pub struct ConnectionAuthority {
    identity: Arc<LeaseIdentity>,
    revoked: watch::Receiver<bool>,
}
impl ConnectionAuthority {
    pub fn account(&self) -> Uuid {
        self.identity.account
    }
    pub fn revoked(&self) -> watch::Receiver<bool> {
        self.revoked.clone()
    }
    pub fn close(&self) {
        if let Some(owner) = self.identity.owner.upgrade() {
            owner.release(self.identity.account, self.identity.token);
        }
    }
    pub fn token(&self) -> Uuid {
        self.identity.token
    }
}
impl Drop for LeaseIdentity {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.upgrade() {
            owner.release(self.account, self.token);
        }
    }
}
struct Barrier(Weak<AuthorityRegistry>);
impl Drop for Barrier {
    fn drop(&mut self) {
        if let Some(owner) = self.0.upgrade()
            && let Ok(mut inner) = owner.inner.lock()
        {
            inner.pending = inner.pending.saturating_sub(1);
            inner.generation = inner.generation.saturating_add(1);
        }
    }
}
impl AuthorityRegistry {
    pub fn new(capacity: usize) -> Result<Arc<Self>, OnlineError> {
        if capacity == 0 {
            return Err(OnlineError::Capacity);
        }
        Ok(Arc::new_cyclic(|owner| Self {
            capacity,
            owner: owner.clone(),
            inner: Mutex::new(Inner {
                generation: 0,
                pending: 0,
                active: HashMap::new(),
            }),
        }))
    }
    pub fn generation(&self) -> Result<u64, OnlineError> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| OnlineError::Unavailable)?
            .generation)
    }
    pub fn is_replacement(&self, lease: &ConnectionAuthority) -> bool {
        self.inner.lock().is_ok_and(|inner| {
            inner
                .active
                .get(&lease.account())
                .is_some_and(|entry| entry.token != lease.token())
        })
    }
    pub fn bind(
        self: &Arc<Self>,
        generation: u64,
        account: Uuid,
        hash: [u8; 32],
        expires: i64,
        now: i64,
    ) -> Result<ConnectionAuthority, OnlineError> {
        if account.is_nil() || hash == [0; 32] || now < 0 || expires <= now {
            return Err(OnlineError::Unauthorized);
        }
        let mut inner = self.inner.lock().map_err(|_| OnlineError::Unavailable)?;
        if generation != inner.generation || inner.pending != 0 || inner.generation == u64::MAX {
            return Err(OnlineError::Unauthorized);
        }
        if inner.active.len() >= self.capacity && !inner.active.contains_key(&account) {
            return Err(OnlineError::Capacity);
        }
        let (closed, revoked) = watch::channel(false);
        let token = Uuid::new_v4();
        if let Some(previous) = inner.active.insert(
            account,
            Entry {
                token,
                hash,
                expires,
                closed,
            },
        ) {
            previous.closed.send_replace(true);
        }
        Ok(ConnectionAuthority {
            identity: Arc::new(LeaseIdentity {
                account,
                token,
                owner: Arc::downgrade(self),
            }),
            revoked,
        })
    }
    pub fn with_authority<T>(
        &self,
        lease: &ConnectionAuthority,
        now: i64,
        operation: impl FnOnce() -> T,
    ) -> Result<T, OnlineError> {
        let mut inner = self.inner.lock().map_err(|_| OnlineError::Unavailable)?;
        let account = lease.account();
        let entry = inner
            .active
            .get(&account)
            .filter(|entry| entry.token == lease.identity.token)
            .ok_or(OnlineError::Unauthorized)?;
        if now < 0 || now >= entry.expires {
            if let Some(entry) = inner.active.remove(&account) {
                entry.closed.send_replace(true);
            }
            inner.generation = inner.generation.saturating_add(1);
            return Err(OnlineError::Unauthorized);
        }
        // Hold the same lock as invalidation through the pure state commit.
        Ok(operation())
    }
    fn release(&self, account: Uuid, token: Uuid) {
        if let Ok(mut inner) = self.inner.lock()
            && inner
                .active
                .get(&account)
                .is_some_and(|entry| entry.token == token)
            && let Some(entry) = inner.active.remove(&account)
        {
            entry.closed.send_replace(true);
        }
    }
    fn invalidate(&self, predicate: impl Fn(&Uuid, &Entry) -> bool) -> InvalidationBarrier {
        if let Ok(mut inner) = self.inner.lock() {
            inner.generation = inner.generation.saturating_add(1);
            inner.pending = inner.pending.saturating_add(1);
            inner.active.retain(|account, entry| {
                if predicate(account, entry) {
                    entry.closed.send_replace(true);
                    false
                } else {
                    true
                }
            });
        }
        Box::new(Barrier(self.owner.clone()))
    }
}
impl SessionInvalidator for AuthorityRegistry {
    fn session(&self, hash: [u8; 32]) -> InvalidationBarrier {
        self.invalidate(|_, entry| entry.hash == hash)
    }
    fn account(&self, account: Uuid) -> InvalidationBarrier {
        self.invalidate(|id, _| *id == account)
    }
}
