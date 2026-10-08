use liar_protocol::online::OnlineError;
use liar_server::{auth::*, online::*};
use std::sync::Arc;
use uuid::Uuid;

fn bind(registry: &Arc<AuthorityRegistry>, account: Uuid, hash: u8) -> ConnectionAuthority {
    registry
        .bind_shared(
            registry.generation().unwrap(),
            account,
            [hash; 32],
            200,
            100,
        )
        .unwrap()
}

#[test]
fn repeated_requests_share_one_identity_until_the_last_owner_drops() {
    let registry = AuthorityRegistry::new(1).unwrap();
    let account = Uuid::new_v4();
    let first = bind(&registry, account, 1);
    let second = bind(&registry, account, 1);
    assert_eq!(first.token(), second.token());
    assert!(!*first.revoked().borrow());
    drop(first);
    assert!(registry.with_authority(&second, 100, || ()).is_ok());
    assert!(
        registry
            .bind_shared(
                registry.generation().unwrap(),
                Uuid::new_v4(),
                [3; 32],
                200,
                100
            )
            .is_err()
    );
    drop(second);
    assert!(
        registry
            .bind_shared(
                registry.generation().unwrap(),
                Uuid::new_v4(),
                [3; 32],
                200,
                100
            )
            .is_ok()
    );
}

#[test]
fn atomic_participant_guard_rejects_expiry_revocation_duplicates_and_foreign_namespace() {
    let registry = AuthorityRegistry::new(2).unwrap();
    let other = AuthorityRegistry::new(2).unwrap();
    let first = bind(&registry, Uuid::new_v4(), 1);
    let second = bind(&registry, Uuid::new_v4(), 2);
    let foreign = bind(&other, first.account(), 1);
    assert_eq!(
        registry.with_authorities(&[&first, &second], 199, || 7),
        Ok(7)
    );
    for leases in [
        vec![],
        vec![&first, &first],
        vec![&foreign],
        vec![&first, &foreign],
    ] {
        assert_eq!(
            registry.with_authorities(&leases, 100, || panic!("unauthorized commit")),
            Err::<(), _>(OnlineError::Unauthorized)
        );
    }
    assert_eq!(
        registry.with_authorities(&[&first, &second], 200, || panic!("expired commit")),
        Err::<(), _>(OnlineError::Unauthorized)
    );
    assert!(*first.revoked().borrow());
    assert!(*second.revoked().borrow());
    assert_eq!(
        registry.with_authorities(&[&first], 199, || ()),
        Err(OnlineError::Unauthorized)
    );
}

#[test]
fn shared_replacement_and_late_drop_do_not_retire_the_new_session() {
    let registry = AuthorityRegistry::new(1).unwrap();
    let old = bind(&registry, Uuid::new_v4(), 1);
    let old_poll = bind(&registry, old.account(), 1);
    let current = bind(&registry, old.account(), 2);
    assert!(*old.revoked().borrow());
    assert!(*old_poll.revoked().borrow());
    assert_eq!(
        registry.with_authorities(&[&old], 100, || ()),
        Err(OnlineError::Unauthorized)
    );
    drop(old);
    drop(old_poll);
    assert_eq!(registry.with_authorities(&[&current], 100, || 7), Ok(7));
    current.close();
    assert_eq!(
        registry.with_authorities(&[&current], 100, || ()),
        Err(OnlineError::Unauthorized)
    );
}

#[test]
fn combined_barriers_reject_stale_and_pending_handshakes_in_both_namespaces() {
    let sockets = AuthorityRegistry::new(2).unwrap();
    let lobby = AuthorityRegistry::new(2).unwrap();
    let invalidator = CombinedSessionInvalidator::new(sockets.clone(), lobby.clone());
    for account_wide in [false, true] {
        let account = Uuid::new_v4();
        let socket = bind(&sockets, account, 1);
        let pending = bind(&lobby, account, 1);
        let other = bind(&lobby, Uuid::new_v4(), 2);
        let before = [sockets.generation().unwrap(), lobby.generation().unwrap()];
        let barrier = if account_wide {
            invalidator.account(account)
        } else {
            invalidator.session([1; 32])
        };
        assert!(*socket.revoked().borrow());
        assert!(*pending.revoked().borrow());
        assert_eq!(lobby.with_authority(&other, 100, || 7), Ok(7));
        let during = [sockets.generation().unwrap(), lobby.generation().unwrap()];
        for (index, registry) in [&sockets, &lobby].into_iter().enumerate() {
            for generation in [before[index], during[index]] {
                assert!(
                    registry
                        .bind_shared(generation, account, [1; 32], 200, 100)
                        .is_err()
                );
            }
        }
        drop(barrier);
        for (index, registry) in [&sockets, &lobby].into_iter().enumerate() {
            assert!(
                registry
                    .bind_shared(during[index], account, [1; 32], 200, 100)
                    .is_err()
            );
            assert!(
                registry
                    .bind_shared(registry.generation().unwrap(), account, [3; 32], 200, 100)
                    .is_ok()
            );
        }
    }
}

#[test]
fn a_revocation_cannot_split_the_two_participant_commit() {
    use std::{sync::mpsc, time::Duration};
    let registry = AuthorityRegistry::new(2).unwrap();
    let first = bind(&registry, Uuid::new_v4(), 1);
    let second = bind(&registry, Uuid::new_v4(), 2);
    let (entered, entry) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (revoking, started) = mpsc::channel();
    let (complete, completed) = mpsc::channel();
    std::thread::scope(|scope| {
        let guard_registry = &registry;
        let guard_first = &first;
        let guard_second = &second;
        scope.spawn(move || {
            guard_registry
                .with_authorities(&[guard_first, guard_second], 100, || {
                    entered.send(()).unwrap();
                    released.recv().unwrap();
                })
                .unwrap();
        });
        entry.recv_timeout(Duration::from_secs(2)).unwrap();
        scope.spawn(|| {
            revoking.send(()).unwrap();
            let _barrier = registry.session([2; 32]);
            complete.send(()).unwrap();
        });
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        let prematurely_completed = completed.recv_timeout(Duration::from_millis(40)).is_ok();
        release.send(()).unwrap();
        assert!(
            !prematurely_completed,
            "revocation entered a partially guarded commit"
        );
    });
    assert!(*second.revoked().borrow());
    assert_eq!(
        registry.with_authorities(&[&first, &second], 100, || ()),
        Err(OnlineError::Unauthorized)
    );
}
