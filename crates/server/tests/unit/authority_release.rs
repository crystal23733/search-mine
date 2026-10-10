use super::*;

#[test]
fn only_the_current_normal_release_sets_its_transport_reason() {
    let registry = AuthorityRegistry::new(1).unwrap();
    let account = Uuid::new_v4();
    let old = registry.bind(0, account, [1; 32], 200, 100).unwrap();
    let fresh = registry.bind(0, account, [2; 32], 200, 100).unwrap();
    old.close();
    assert!(!old.was_released());
    assert!(!fresh.was_released());
    assert!(registry.with_authority(&fresh, 100, || ()).is_ok());
    fresh.close();
    assert!(fresh.was_released());
    assert!(registry.with_authority(&fresh, 100, || ()).is_err());
}

#[test]
fn invalidation_or_expiry_before_release_cannot_be_reclassified_as_normal() {
    for expire in [false, true] {
        let registry = AuthorityRegistry::new(1).unwrap();
        let account = Uuid::new_v4();
        let lease = registry.bind(0, account, [1; 32], 200, 100).unwrap();
        if expire {
            assert!(registry.with_authority(&lease, 200, || ()).is_err());
        } else {
            drop(registry.account(account));
        }
        lease.close();
        assert!(!lease.was_released());
        assert!(*lease.revoked().borrow());
        assert!(registry.with_authority(&lease, 100, || ()).is_err());
    }
}
