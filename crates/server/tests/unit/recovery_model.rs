use super::*;
use serde_json::{Value, json};
const NOW: i64 = 1_800_000_000;
fn valid() -> Value {
    json!({"version":1,"recovery_id":Uuid::new_v4(),"captured_at":NOW,"entries":[{"kind":"account","id":Uuid::new_v4(),"deleted_at":NOW-10,"expires_at":NOW-10+2419200}]})
}
fn parse(v: &Value, now: i64) -> Result<Ledger, RecoveryError> {
    Ledger::parse(&serde_json::to_vec(v).unwrap(), now)
}
#[test]
fn validated_empty_and_account_identity_batches_are_accepted_without_an_activation_field() {
    let mut v = valid();
    assert!(parse(&v, NOW).is_ok());
    v["entries"][0]["kind"] = json!("identity");
    assert!(parse(&v, NOW).is_ok());
    v["entries"] = json!([]);
    assert!(parse(&v, NOW).is_ok());
}
#[test]
fn capture_and_expiry_boundaries_are_exact() {
    let v = valid();
    assert!(parse(&v, NOW + 300).is_ok());
    assert!(parse(&v, NOW + 301).is_err());
    assert!(parse(&v, NOW - 1).is_err());
    assert!(parse(&v, -1).is_err());
    assert!(parse(&v, 253402300800).is_err());
    let mut v = valid();
    v["entries"][0]["deleted_at"] = json!(NOW - 2419200);
    v["entries"][0]["expires_at"] = json!(NOW);
    assert!(parse(&v, NOW).is_err());
    v["entries"][0]["deleted_at"] = json!(NOW - 2419200 + 1);
    v["entries"][0]["expires_at"] = json!(NOW + 1);
    assert!(parse(&v, NOW).is_ok());
}
#[test]
fn malformed_authority_and_unknown_fields_never_acquire_access() {
    for (field, value) in [
        ("version", json!(2)),
        ("recovery_id", json!(Uuid::nil())),
        ("captured_at", json!(NOW + 1)),
        ("captured_at", json!(-1)),
        ("activate", json!(true)),
    ] {
        let mut v = valid();
        v[field] = value;
        assert!(parse(&v, NOW).is_err());
    }
    for (field, value) in [
        ("kind", json!("session")),
        ("id", json!(Uuid::nil())),
        ("deleted_at", json!(NOW + 1)),
        ("deleted_at", json!(-1)),
        ("expires_at", json!(NOW)),
        ("expires_at", json!(253402300800i64)),
        ("nickname", json!("forbidden")),
    ] {
        let mut v = valid();
        v["entries"][0][field] = value;
        assert!(parse(&v, NOW).is_err());
    }
    assert!(Ledger::parse(b"{}", NOW).is_err());
    assert!(Ledger::parse(b"invalid", NOW).is_err());
    assert!(Ledger::parse(&vec![b' '; MAX_BYTES + 1], NOW).is_err());
}
#[test]
fn batch_size_and_duplicate_erasure_keys_are_bounded() {
    let mut v = valid();
    let entry = v["entries"][0].clone();
    v["entries"] = json!([entry.clone(), entry.clone()]);
    assert!(parse(&v, NOW).is_err());
    v["entries"][1]["kind"] = json!("identity");
    assert!(parse(&v, NOW).is_ok());
    let entries = (0..500)
        .map(|_| {
            let mut e = entry.clone();
            e["id"] = json!(Uuid::new_v4());
            e
        })
        .collect::<Vec<_>>();
    v["entries"] = json!(entries);
    assert!(parse(&v, NOW).is_ok());
    v["entries"].as_array_mut().unwrap().push(entry);
    assert!(parse(&v, NOW).is_err());
}
