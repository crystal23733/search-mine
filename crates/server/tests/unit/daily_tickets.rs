use super::*;
use liar_core::rules::RulesSnapshot;
use std::sync::atomic::{AtomicU64, Ordering};

struct Clock(AtomicU64);
impl MatchClock for Clock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
#[test]
fn identical_start_keeps_original_ticket_and_deadline() {
    let authorities = AuthorityRegistry::new(8).unwrap();
    let clock = Arc::new(Clock(AtomicU64::new(1000)));
    let registry = DailyTicketRegistry::new(8, authorities.clone(), clock.clone()).unwrap();
    let account = Uuid::new_v4();
    let lease = authorities
        .bind_shared(
            authorities.generation().unwrap(),
            account,
            [1; 32],
            1000,
            100,
        )
        .unwrap();
    let revision = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    let definition = PublishedDaily::new("2026-10-07", 1, RulesSnapshot::bundled()).unwrap();
    let first = registry
        .start(&lease, revision, attempt, &definition, 100)
        .unwrap();
    clock.0.store(2000, Ordering::SeqCst);
    assert_eq!(
        registry
            .start(&lease, revision, attempt, &definition, 101)
            .unwrap(),
        first
    );
    assert!(!first.id.is_nil());
    assert_eq!(
        first.ttl_ms,
        u64::from(definition.rules().rules.countdown_ms)
            + u64::from(definition.rules().rules.duration_ms)
    );
    assert_eq!(first.heartbeat_interval_ms, 15000);
}

fn definition() -> PublishedDaily {
    PublishedDaily::new("2026-10-07", 1, RulesSnapshot::bundled()).unwrap()
}
fn evidence() -> ReplayEvidence {
    use crate::daily::{NativeReplayVerifier, ReplayVerifier};
    let value: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    NativeReplayVerifier
        .verify(&definition(), &value["daily"]["replay"].to_string())
        .unwrap()
}
struct Setup {
    authorities: Arc<AuthorityRegistry>,
    clock: Arc<Clock>,
    registry: Arc<DailyTicketRegistry>,
    lease: ConnectionAuthority,
    revision: Uuid,
    attempt: Uuid,
}
impl Setup {
    fn new(capacity: usize) -> Self {
        let authorities = AuthorityRegistry::new(32).unwrap();
        let clock = Arc::new(Clock(AtomicU64::new(1000)));
        let registry = Arc::new(
            DailyTicketRegistry::new(capacity, authorities.clone(), clock.clone()).unwrap(),
        );
        let lease = authorities
            .bind_shared(
                authorities.generation().unwrap(),
                Uuid::new_v4(),
                [1; 32],
                1000,
                100,
            )
            .unwrap();
        Self {
            authorities,
            clock,
            registry,
            lease,
            revision: Uuid::new_v4(),
            attempt: Uuid::new_v4(),
        }
    }
    fn start(&self) -> TicketGrant {
        self.registry
            .start(&self.lease, self.revision, self.attempt, &definition(), 100)
            .unwrap()
    }
    fn set(&self, now: u64) {
        self.clock.0.store(now, Ordering::SeqCst);
    }
    fn finish(&self, grant: &TicketGrant) -> Result<Option<VerifiedDailyTime>, TicketError> {
        self.registry.finish(
            &self.lease,
            self.revision,
            grant.id,
            grant.attempt,
            &evidence(),
            100,
        )
    }
    fn other(&self) -> ConnectionAuthority {
        self.authorities
            .bind_shared(
                self.authorities.generation().unwrap(),
                Uuid::new_v4(),
                [2; 32],
                1000,
                100,
            )
            .unwrap()
    }
}

#[test]
fn expiring_last_owned_lease_does_not_reenter_the_locked_authority_registry() {
    let s = Setup::new(8);
    let other = s.other();
    let grant = s
        .registry
        .start(&other, Uuid::new_v4(), Uuid::new_v4(), &definition(), 100)
        .unwrap();
    drop(other);
    s.set(1000 + grant.ttl_ms);
    let registry = s.registry.clone();
    let lease = s.lease.clone();
    let (sent, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        sent.send(registry.start(&lease, Uuid::new_v4(), Uuid::new_v4(), &definition(), 100))
            .unwrap();
    });
    assert!(
        received
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap()
            .is_ok()
    );
}

#[test]
fn expired_ticket_reaping_frees_authority_capacity_before_a_new_account_can_bind() {
    let authorities = AuthorityRegistry::new(1).unwrap();
    let clock = Arc::new(Clock(AtomicU64::new(1000)));
    let registry = DailyTicketRegistry::new(1, authorities.clone(), clock.clone()).unwrap();
    let original = authorities
        .bind_shared(
            authorities.generation().unwrap(),
            Uuid::new_v4(),
            [1; 32],
            1000,
            100,
        )
        .unwrap();
    let grant = registry
        .start(
            &original,
            Uuid::new_v4(),
            Uuid::new_v4(),
            &definition(),
            100,
        )
        .unwrap();
    drop(original);
    registry.reap().unwrap();
    assert_eq!(registry.inner.lock().unwrap().entries.len(), 1);
    clock.0.store(1000 + grant.ttl_ms, Ordering::SeqCst);
    let next_account = Uuid::new_v4();
    assert!(matches!(
        authorities.bind_shared(
            authorities.generation().unwrap(),
            next_account,
            [2; 32],
            1000,
            100
        ),
        Err(OnlineError::Capacity)
    ));
    registry.reap().unwrap();
    let next = authorities
        .bind_shared(
            authorities.generation().unwrap(),
            next_account,
            [2; 32],
            1000,
            100,
        )
        .unwrap();
    assert!(
        registry
            .start(&next, Uuid::new_v4(), Uuid::new_v4(), &definition(), 100)
            .is_ok()
    );
}

#[test]
fn ticket_retains_the_lease_across_request_lifetimes_and_late_finish_cannot_target_replacement() {
    let s = Setup::new(8);
    let lease = s.other();
    let account = lease.account();
    let token = lease.token();
    let revision = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    let first = s
        .registry
        .start(&lease, revision, attempt, &definition(), 100)
        .unwrap();
    drop(lease);
    let next = s
        .authorities
        .bind_shared(
            s.authorities.generation().unwrap(),
            account,
            [2; 32],
            1000,
            100,
        )
        .unwrap();
    assert_eq!(next.token(), token);
    let second = s
        .registry
        .start(&next, revision, Uuid::new_v4(), &definition(), 100)
        .unwrap();
    let registry = s.registry.clone();
    let current = next.clone();
    let proof = evidence();
    let (release, wait) = std::sync::mpsc::channel();
    let delayed = std::thread::spawn(move || {
        wait.recv().unwrap();
        registry.finish(&current, revision, first.id, first.attempt, &proof, 100)
    });
    s.set(15000);
    release.send(()).unwrap();
    assert_eq!(delayed.join().unwrap().unwrap(), None);
    assert!(
        s.registry
            .finish(&next, revision, second.id, second.attempt, &evidence(), 100)
            .unwrap()
            .is_some()
    );
}
#[test]
fn clock_failure_and_revocation_retire_last_owned_leases_outside_both_locks() {
    for revoke in [false, true] {
        let s = Setup::new(8);
        let other = s.other();
        let account = other.account();
        s.registry
            .start(&other, Uuid::new_v4(), Uuid::new_v4(), &definition(), 100)
            .unwrap();
        drop(other);
        let registry = s.registry.clone();
        let lease = s.lease.clone();
        if !revoke {
            s.set(999);
        }
        let (sent, received) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            if revoke {
                drop(registry.account(account));
                sent.send(Ok(())).unwrap();
            } else {
                sent.send(
                    registry
                        .start(&lease, Uuid::new_v4(), Uuid::new_v4(), &definition(), 100)
                        .map(|_| ()),
                )
                .unwrap();
            }
        });
        let result = received
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap();
        if revoke {
            assert_eq!(result, Ok(()));
        } else {
            assert_eq!(result, Err(TicketError::Unavailable));
        }
        assert!(s.registry.inner.lock().unwrap().entries.is_empty());
    }
}
#[test]
fn bounded_capacity_replacement_expiry_and_missing_tickets_preserve_one_per_account() {
    let s = Setup::new(1);
    let first = s.start();
    let other = s.other();
    assert_eq!(
        s.registry
            .start(&other, Uuid::new_v4(), Uuid::new_v4(), &definition(), 100),
        Err(TicketError::Capacity)
    );
    let second = s
        .registry
        .start(&s.lease, s.revision, Uuid::new_v4(), &definition(), 100)
        .unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(s.registry.inner.lock().unwrap().entries.len(), 1);
    assert_eq!(s.finish(&first).unwrap(), None);
    assert_eq!(
        s.registry
            .heartbeat(&s.lease, s.revision, first.id, 1, 100)
            .unwrap(),
        None
    );
    s.set(1000 + second.ttl_ms);
    assert_eq!(s.finish(&second).unwrap(), None);
    assert!(
        s.registry
            .start(&other, Uuid::new_v4(), Uuid::new_v4(), &definition(), 100)
            .is_ok()
    );
    for capacity in [0, 20001, usize::MAX] {
        assert!(matches!(
            DailyTicketRegistry::new(capacity, s.authorities.clone(), s.clock.clone()),
            Err(TicketError::Capacity)
        ));
    }
    assert!(DailyTicketRegistry::new(20000, s.authorities.clone(), s.clock.clone()).is_ok());
}
#[test]
fn identity_revision_metadata_and_attempt_are_bound_before_mutation() {
    let s = Setup::new(8);
    let grant = s.start();
    let other = s.other();
    assert_eq!(
        s.registry
            .start(&s.lease, Uuid::nil(), s.attempt, &definition(), 100),
        Err(TicketError::Invalid)
    );
    assert_eq!(
        s.registry
            .start(&s.lease, s.revision, Uuid::nil(), &definition(), 100),
        Err(TicketError::Invalid)
    );
    assert_eq!(
        s.registry
            .start(&s.lease, Uuid::new_v4(), s.attempt, &definition(), 100),
        Err(TicketError::Conflict)
    );
    let other_definition = PublishedDaily::new("2026-10-08", 1, RulesSnapshot::bundled()).unwrap();
    assert_eq!(
        s.registry
            .start(&s.lease, s.revision, s.attempt, &other_definition, 100),
        Err(TicketError::Conflict)
    );
    for (revision, ticket, sequence) in [
        (Uuid::nil(), grant.id, 1),
        (s.revision, Uuid::nil(), 1),
        (s.revision, grant.id, 0),
    ] {
        assert_eq!(
            s.registry
                .heartbeat(&s.lease, revision, ticket, sequence, 100),
            Err(TicketError::Invalid)
        );
    }
    for (revision, ticket, attempt) in [
        (Uuid::nil(), grant.id, s.attempt),
        (s.revision, Uuid::nil(), s.attempt),
        (s.revision, grant.id, Uuid::nil()),
    ] {
        assert_eq!(
            s.registry
                .finish(&s.lease, revision, ticket, attempt, &evidence(), 100),
            Err(TicketError::Invalid)
        );
    }
    assert_eq!(
        s.registry
            .heartbeat(&s.lease, Uuid::new_v4(), grant.id, 1, 100),
        Err(TicketError::Unauthorized)
    );
    assert_eq!(
        s.registry.finish(
            &s.lease,
            Uuid::new_v4(),
            grant.id,
            s.attempt,
            &evidence(),
            100
        ),
        Err(TicketError::Unauthorized)
    );
    assert_eq!(
        s.registry.finish(
            &s.lease,
            s.revision,
            grant.id,
            Uuid::new_v4(),
            &evidence(),
            100
        ),
        Err(TicketError::Conflict)
    );
    assert_eq!(
        s.registry
            .finish(&other, s.revision, grant.id, s.attempt, &evidence(), 100)
            .unwrap(),
        None
    );
    s.set(15000);
    let saved = s.finish(&grant).unwrap().unwrap();
    assert_eq!(saved.elapsed_ms(), 11000);
    assert!(saved.matches(s.lease.account(), s.attempt, &evidence()));
}
#[test]
fn heartbeat_gap_start_retry_and_registry_restart_never_restore_time() {
    let s = Setup::new(8);
    let grant = s.start();
    s.set(16000);
    let ack = s
        .registry
        .heartbeat(&s.lease, s.revision, grant.id, 1, 100)
        .unwrap()
        .unwrap();
    s.set(59000);
    assert_eq!(
        s.registry
            .heartbeat(&s.lease, s.revision, grant.id, 1, 100)
            .unwrap(),
        Some(ack)
    );
    s.set(61000);
    assert_eq!(
        s.registry
            .heartbeat(&s.lease, s.revision, grant.id, 2, 100)
            .unwrap(),
        None
    );
    assert_eq!(s.start(), grant);
    assert_eq!(s.finish(&grant).unwrap(), None);
    let restarted = DailyTicketRegistry::new(8, s.authorities.clone(), s.clock.clone()).unwrap();
    assert_eq!(
        restarted
            .finish(&s.lease, s.revision, grant.id, s.attempt, &evidence(), 100)
            .unwrap(),
        None
    );
    assert_eq!(
        restarted
            .heartbeat(&s.lease, s.revision, grant.id, 2, 100)
            .unwrap(),
        None
    );
}
#[test]
fn foreign_expired_released_and_replaced_leases_cannot_mutate_ticket_state() {
    let s = Setup::new(8);
    let grant = s.start();
    let foreign = AuthorityRegistry::new(8).unwrap();
    let foreign_lease = foreign
        .bind_shared(
            foreign.generation().unwrap(),
            s.lease.account(),
            [1; 32],
            1000,
            100,
        )
        .unwrap();
    assert_eq!(
        s.registry
            .start(&foreign_lease, s.revision, s.attempt, &definition(), 100),
        Err(TicketError::Unauthorized)
    );
    let replacement = s
        .authorities
        .bind_shared(
            s.authorities.generation().unwrap(),
            s.lease.account(),
            [3; 32],
            1000,
            100,
        )
        .unwrap();
    assert_eq!(
        s.registry
            .start(&s.lease, s.revision, s.attempt, &definition(), 100),
        Err(TicketError::Unauthorized)
    );
    assert_eq!(
        s.registry
            .finish(
                &replacement,
                s.revision,
                grant.id,
                s.attempt,
                &evidence(),
                100
            )
            .unwrap(),
        None
    );
    let newer = s
        .registry
        .start(
            &replacement,
            Uuid::new_v4(),
            Uuid::new_v4(),
            &definition(),
            100,
        )
        .unwrap();
    assert_ne!(newer.id, grant.id);
    replacement.close();
    assert_eq!(
        s.registry
            .start(&replacement, s.revision, Uuid::new_v4(), &definition(), 100),
        Err(TicketError::Unauthorized)
    );
    let expired = Setup::new(8);
    expired.start();
    assert_eq!(
        expired
            .registry
            .heartbeat(&expired.lease, expired.revision, Uuid::new_v4(), 1, 1000),
        Err(TicketError::Unauthorized)
    );
}
#[test]
fn session_and_account_invalidation_hold_actual_barrier_through_late_bind() {
    for account in [false, true] {
        let s = Setup::new(8);
        let grant = s.start();
        let other = s.other();
        let other_revision = Uuid::new_v4();
        let other_ticket = s
            .registry
            .start(&other, other_revision, Uuid::new_v4(), &definition(), 100)
            .unwrap();
        let stale = s.authorities.generation().unwrap();
        let barrier = if account {
            s.registry.account(s.lease.account())
        } else {
            s.registry.session([1; 32])
        };
        assert_eq!(s.registry.inner.lock().unwrap().entries.len(), 1);
        assert_eq!(s.finish(&grant), Err(TicketError::Unauthorized));
        for generation in [stale, s.authorities.generation().unwrap()] {
            assert!(matches!(
                s.authorities
                    .bind_shared(generation, s.lease.account(), [1; 32], 1000, 100),
                Err(OnlineError::Unauthorized)
            ));
        }
        s.set(15000);
        assert!(
            s.registry
                .finish(
                    &other,
                    other_revision,
                    other_ticket.id,
                    other_ticket.attempt,
                    &evidence(),
                    100
                )
                .unwrap()
                .is_some()
        );
        drop(barrier);
        assert!(matches!(
            s.authorities
                .bind_shared(stale, s.lease.account(), [1; 32], 1000, 100),
            Err(OnlineError::Unauthorized)
        ));
        let current = s
            .authorities
            .bind_shared(
                s.authorities.generation().unwrap(),
                s.lease.account(),
                [1; 32],
                1000,
                100,
            )
            .unwrap();
        assert_eq!(
            s.registry
                .finish(&current, s.revision, grant.id, s.attempt, &evidence(), 100)
                .unwrap(),
            None
        );
    }
}
#[test]
fn monotonic_reversal_and_overflow_fail_closed_without_cached_proof() {
    let s = Setup::new(8);
    let grant = s.start();
    s.set(15000);
    s.finish(&grant).unwrap().unwrap();
    s.set(14999);
    assert_eq!(s.finish(&grant), Err(TicketError::Unavailable));
    assert!(s.registry.inner.lock().unwrap().entries.is_empty());
    s.set(16000);
    assert_eq!(s.start_result(), Err(TicketError::Unavailable));
    let s = Setup::new(8);
    s.set(u64::MAX);
    assert_eq!(s.start_result(), Err(TicketError::Unavailable));
    assert!(s.registry.inner.lock().unwrap().entries.is_empty());
}
impl Setup {
    fn start_result(&self) -> Result<TicketGrant, TicketError> {
        self.registry
            .start(&self.lease, self.revision, self.attempt, &definition(), 100)
    }
}
#[test]
fn poisoned_memory_is_unavailable_and_invalidation_still_revokes_authority() {
    let s = Setup::new(8);
    let grant = s.start();
    let registry = s.registry.clone();
    assert!(
        std::thread::spawn(move || {
            let _guard = registry.inner.lock().unwrap();
            panic!("injected ticket lock panic");
        })
        .join()
        .is_err()
    );
    assert_eq!(s.finish(&grant), Err(TicketError::Unavailable));
    assert_eq!(s.registry.reap(), Err(TicketError::Unavailable));
    drop(s.registry.account(s.lease.account()));
    assert_eq!(s.finish(&grant), Err(TicketError::Unauthorized));
    assert_eq!(
        authority(OnlineError::Unavailable),
        TicketError::Unavailable
    );
}

#[test]
fn reap_clock_reversal_is_permanent_and_cannot_create_a_new_proof() {
    let s = Setup::new(8);
    let grant = s.start();
    s.set(15000);
    s.finish(&grant).unwrap().unwrap();
    s.set(14999);
    assert_eq!(s.registry.reap(), Err(TicketError::Unavailable));
    assert!(s.registry.inner.lock().unwrap().entries.is_empty());
    s.set(16000);
    assert_eq!(s.registry.reap(), Err(TicketError::Unavailable));
    assert_eq!(s.start_result(), Err(TicketError::Unavailable));
}
#[test]
fn concurrent_start_and_finish_share_one_original_ticket_and_server_observation() {
    let s = Setup::new(8);
    let barrier = Arc::new(std::sync::Barrier::new(12));
    let starts = (0..12)
        .map(|_| {
            let registry = s.registry.clone();
            let lease = s.lease.clone();
            let barrier = barrier.clone();
            let revision = s.revision;
            let attempt = s.attempt;
            std::thread::spawn(move || {
                barrier.wait();
                registry
                    .start(&lease, revision, attempt, &definition(), 100)
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let grants = starts
        .into_iter()
        .map(|task| task.join().unwrap())
        .collect::<Vec<_>>();
    assert!(grants.iter().all(|grant| grant == &grants[0]));
    s.set(15000);
    let proof = Arc::new(evidence());
    let finishes = (0..12)
        .map(|_| {
            let registry = s.registry.clone();
            let lease = s.lease.clone();
            let barrier = barrier.clone();
            let revision = s.revision;
            let grant = grants[0].clone();
            let proof = proof.clone();
            std::thread::spawn(move || {
                barrier.wait();
                registry
                    .finish(&lease, revision, grant.id, grant.attempt, &proof, 100)
                    .unwrap()
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let times = finishes
        .into_iter()
        .map(|task| task.join().unwrap())
        .collect::<Vec<_>>();
    assert!(times.iter().all(|time| time == &times[0]));
    assert_eq!(times[0].elapsed_ms(), 11000);
    assert_eq!(s.registry.inner.lock().unwrap().entries.len(), 1);
}
#[test]
fn concurrent_revocation_linearizes_before_or_after_finish_without_late_recreation() {
    for _ in 0..16 {
        let s = Setup::new(8);
        let grant = s.start();
        s.set(15000);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let registry = s.registry.clone();
        let lease = s.lease.clone();
        let revision = s.revision;
        let gate = barrier.clone();
        let proof = evidence();
        let finish = std::thread::spawn(move || {
            gate.wait();
            registry.finish(&lease, revision, grant.id, grant.attempt, &proof, 100)
        });
        barrier.wait();
        let invalidation = s.registry.account(s.lease.account());
        let result = finish.join().unwrap();
        assert!(matches!(
            result,
            Ok(Some(_)) | Err(TicketError::Unauthorized)
        ));
        assert!(s.registry.inner.lock().unwrap().entries.is_empty());
        assert_eq!(s.start_result(), Err(TicketError::Unauthorized));
        drop(invalidation);
    }
}
