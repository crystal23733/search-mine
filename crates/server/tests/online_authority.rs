use liar_server::{auth::SessionInvalidator, online::AuthorityRegistry};
use uuid::Uuid;
#[test]
fn independent_handshakes_share_the_same_revocation_snapshot_without_rejecting_each_other() {
    let registry = AuthorityRegistry::new(2).unwrap();
    let generation = registry.generation().unwrap();
    let first = registry
        .bind(generation, Uuid::new_v4(), [1; 32], 200, 100)
        .unwrap();
    let second = registry
        .bind(generation, Uuid::new_v4(), [2; 32], 200, 100)
        .expect("Unrelated registration must not invalidate a valid session snapshot");
    assert!(registry.with_authority(&first, 100, || ()).is_ok());
    assert!(registry.with_authority(&second, 100, || ()).is_ok());
}

#[test]
fn revocation_rejects_queued_work_and_handshakes_started_before_or_during_commit() {
    let registry = AuthorityRegistry::new(2).unwrap();
    let account = Uuid::new_v4();
    let before = registry.generation().unwrap();
    let lease = registry.bind(before, account, [46; 32], 200, 100).unwrap();
    assert_eq!(registry.with_authority(&lease, 100, || 7).unwrap(), 7);
    let stale_handshake = registry.generation().unwrap();
    let barrier = registry.session([46; 32]);
    assert!(
        registry
            .with_authority(&lease, 100, || "must not commit")
            .is_err()
    );
    assert!(*lease.revoked().borrow());
    assert!(
        registry
            .bind(stale_handshake, account, [46; 32], 200, 100)
            .is_err()
    );
    let during_commit = registry.generation().unwrap();
    assert!(
        registry
            .bind(during_commit, account, [46; 32], 200, 100)
            .is_err()
    );
    drop(barrier);
    assert!(
        registry
            .bind(during_commit, account, [46; 32], 200, 100)
            .is_err()
    );
    let fresh = registry.generation().unwrap();
    assert!(registry.bind(fresh, account, [47; 32], 200, 100).is_ok());
}

#[test]
fn replacing_owner_preserves_new_connection_on_late_old_drop_and_caps_distinct_accounts() {
    let registry = AuthorityRegistry::new(1).unwrap();
    let account = Uuid::new_v4();
    let old = registry
        .bind(registry.generation().unwrap(), account, [1; 32], 200, 100)
        .unwrap();
    let current = registry
        .bind(registry.generation().unwrap(), account, [2; 32], 200, 100)
        .unwrap();
    assert!(registry.with_authority(&old, 100, || ()).is_err());
    drop(old);
    assert!(registry.with_authority(&current, 100, || ()).is_ok());
    assert!(
        registry
            .bind(
                registry.generation().unwrap(),
                Uuid::new_v4(),
                [3; 32],
                200,
                100
            )
            .is_err()
    );
    drop(current);
    assert!(
        registry
            .bind(
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
fn exact_expiry_and_account_revocation_remove_authority_without_affecting_other_accounts() {
    let registry = AuthorityRegistry::new(2).unwrap();
    let account = Uuid::new_v4();
    let first = registry
        .bind(registry.generation().unwrap(), account, [1; 32], 200, 100)
        .unwrap();
    let second = registry
        .bind(
            registry.generation().unwrap(),
            Uuid::new_v4(),
            [2; 32],
            200,
            100,
        )
        .unwrap();
    let barrier = registry.account(account);
    assert!(registry.with_authority(&first, 100, || ()).is_err());
    assert!(registry.with_authority(&second, 100, || ()).is_ok());
    drop(barrier);
    assert!(registry.with_authority(&second, 199, || ()).is_ok());
    assert!(registry.with_authority(&second, 200, || ()).is_err());
    assert!(registry.with_authority(&second, 100, || ()).is_err());
}
