use crate::auth::{InvalidationBarrier, SessionInvalidator};
use liar_protocol::online::OnlineError;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
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
    identity: Weak<LeaseIdentity>,
}
struct Binding {
    generation: u64,
    account: Uuid,
    hash: [u8; 32],
    expires: i64,
    now: i64,
}
struct LeaseIdentity {
    account: Uuid,
    token: Uuid,
    owner: Weak<AuthorityRegistry>,
    released: AtomicBool,
}
#[derive(Clone)]
pub struct ConnectionAuthority {
    identity: Arc<LeaseIdentity>,
    revoked: watch::Receiver<bool>,
}
impl ConnectionAuthority {
    pub(super) fn was_released(&self) -> bool {
        self.identity.released.load(Ordering::Acquire)
    }
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
        self.bind_inner(
            Binding {
                generation,
                account,
                hash,
                expires,
                now,
            },
            false,
        )
    }
    fn bind_inner(
        self: &Arc<Self>,
        binding: Binding,
        shared: bool,
    ) -> Result<ConnectionAuthority, OnlineError> {
        let Binding {
            generation,
            account,
            hash,
            expires,
            now,
        } = binding;
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
        if shared
            && let Some(entry) = inner.active.get(&account)
            && entry.hash == hash
            && entry.expires == expires
            && let Some(identity) = entry.identity.upgrade()
        {
            return Ok(ConnectionAuthority {
                identity,
                revoked: entry.closed.subscribe(),
            });
        }
        let (closed, revoked) = watch::channel(false);
        let token = Uuid::new_v4();
        let identity = Arc::new(LeaseIdentity {
            account,
            token,
            owner: Arc::downgrade(self),
            released: AtomicBool::new(false),
        });
        if let Some(previous) = inner.active.insert(
            account,
            Entry {
                token,
                hash,
                expires,
                closed,
                identity: Arc::downgrade(&identity),
            },
        ) {
            previous.closed.send_replace(true);
        }
        Ok(ConnectionAuthority { identity, revoked })
    }
    /// Bind a verified session using the generation captured before reading storage.
    /// Repeated requests share ownership; a different hash or expiry replaces it.
    pub fn bind_shared(
        self: &Arc<Self>,
        generation: u64,
        account: Uuid,
        hash: [u8; 32],
        expires: i64,
        now: i64,
    ) -> Result<ConnectionAuthority, OnlineError> {
        self.bind_inner(
            Binding {
                generation,
                account,
                hash,
                expires,
                now,
            },
            true,
        )
    }
    /// Validate all participants and hold revocation out through a pure commit.
    /// The operation must not reenter this registry or perform blocking IO.
    pub fn with_authorities<T>(
        &self,
        leases: &[&ConnectionAuthority],
        now: i64,
        operation: impl FnOnce() -> T,
    ) -> Result<T, OnlineError> {
        if !(1..=2).contains(&leases.len())
            || now < 0
            || (leases.len() == 2 && leases[0].account() == leases[1].account())
        {
            return Err(OnlineError::Unauthorized);
        }
        let mut inner = self.inner.lock().map_err(|_| OnlineError::Unavailable)?;
        let mut expired = Vec::new();
        for lease in leases {
            if !Weak::ptr_eq(&lease.identity.owner, &self.owner) {
                return Err(OnlineError::Unauthorized);
            }
            let entry = inner
                .active
                .get(&lease.account())
                .filter(|entry| entry.token == lease.identity.token)
                .ok_or(OnlineError::Unauthorized)?;
            if now >= entry.expires {
                expired.push(lease.account());
            }
        }
        if !expired.is_empty() {
            for account in expired {
                if let Some(entry) = inner.active.remove(&account) {
                    entry.closed.send_replace(true);
                }
            }
            inner.generation = inner.generation.saturating_add(1);
            return Err(OnlineError::Unauthorized);
        }
        // Revocation uses this same lock, including the final pure state commit.
        Ok(operation())
    }
    pub fn with_authority<T>(
        &self,
        lease: &ConnectionAuthority,
        now: i64,
        operation: impl FnOnce() -> T,
    ) -> Result<T, OnlineError> {
        self.with_authorities(&[lease], now, operation)
    }
    fn release(&self, account: Uuid, token: Uuid) {
        if let Ok(mut inner) = self.inner.lock()
            && inner
                .active
                .get(&account)
                .is_some_and(|entry| entry.token == token)
            && let Some(entry) = inner.active.remove(&account)
        {
            if let Some(identity) = entry.identity.upgrade() {
                identity.released.store(true, Ordering::Release);
            }
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
#[cfg(test)]
#[path = "../../tests/unit/authority_release.rs"]
mod release_tests;
impl SessionInvalidator for AuthorityRegistry {
    fn session(&self, hash: [u8; 32]) -> InvalidationBarrier {
        self.invalidate(|_, entry| entry.hash == hash)
    }
    fn account(&self, account: Uuid) -> InvalidationBarrier {
        self.invalidate(|id, _| *id == account)
    }
}
