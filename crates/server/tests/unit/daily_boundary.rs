use super::*;
use crate::auth::{Account, Nickname, Session};
use liar_core::rules::RulesSnapshot;
use serde_json::{Value, json};
const NOW: i64 = 1_791_331_260;
fn sample() -> Value {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    fixture["daily"]["replay"].clone()
}
fn request(operation: Operation) -> Value {
    let mut value = json!({"v":1,"account_id":Uuid::new_v4().to_string(),"session_revision":Uuid::new_v4().to_string()});
    match operation {
        Operation::Definition => {
            value["date"] = json!("2026-10-07");
            value["seed_version"] = json!(1);
        }
        Operation::Own => value["metadata"] = sample()["metadata"].clone(),
        Operation::Submit => {
            value["attempt_id"] = json!(Uuid::new_v4().to_string());
            value["replay"] = sample();
        }
    }
    value
}
fn parse(value: Value, operation: Operation) -> Result<Expected, ReplayError> {
    decode(serde_json::to_vec(&value).unwrap(), operation)
}
#[test]
fn closed_requests_have_canonical_identity_version_and_bounded_replay() {
    for operation in [Operation::Definition, Operation::Submit, Operation::Own] {
        let valid = request(operation);
        let parsed = parse(valid.clone(), operation).unwrap();
        assert_eq!(parsed.account.to_string(), valid["account_id"]);
        for field in ["account_id", "session_revision"] {
            for bad in [
                "",
                "00000000-0000-0000-0000-000000000000",
                "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
            ] {
                let mut value = valid.clone();
                value[field] = json!(bad);
                assert!(parse(value, operation).is_err());
            }
        }
        let mut extra = valid.clone();
        extra["extra"] = json!(true);
        assert!(parse(extra, operation).is_err());
        let mut version = valid.clone();
        version["v"] = json!(2);
        assert!(parse(version, operation).is_err());
        assert!(decode(vec![b' '; 2 * 1024 * 1024 + 4097], operation).is_err());
        assert!(decode(b"{".to_vec(), operation).is_err());
    }
    let mut value = request(Operation::Submit);
    value["attempt_id"] = json!(Uuid::nil().to_string());
    assert!(parse(value, Operation::Submit).is_err());
    let mut value = request(Operation::Submit);
    value["replay"]["inputs"][0]["input"]["secret"] = json!(1);
    assert!(parse(value, Operation::Submit).is_err());
    let mut value = request(Operation::Submit);
    value["replay"]["v"] = json!(2);
    assert!(parse(value, Operation::Submit).is_err());
    let mut value = request(Operation::Submit);
    value["replay"]["inputs"][0]["input"]["action"] = json!({"type":"open","cell":0,"secret":1});
    assert!(parse(value, Operation::Submit).is_err());
    let mut value = request(Operation::Definition);
    value["date"] = json!("2026-02-30");
    assert!(parse(value, Operation::Definition).is_err());
    let mut value = request(Operation::Own);
    value["metadata"]["seed_version"] = json!(99);
    assert!(parse(value, Operation::Own).is_err());
    let mut value = request(Operation::Submit);
    let entry = value["replay"]["inputs"][0].clone();
    value["replay"]["inputs"] = Value::Array(vec![
        entry;
        usize::from(
            RulesSnapshot::bundled().rules.max_commands_per_seat
        ) + 1
    ]);
    assert!(parse(value, Operation::Submit).is_err());
    let mut value = request(Operation::Submit);
    value["replay"]["metadata"]["seed"] = json!("x".repeat(MAX_DAILY_REPLAY_BYTES));
    assert!(parse(value, Operation::Submit).is_err());
}
#[test]
fn proof_requires_current_game_identity_but_never_a_five_minute_reauthentication() {
    let expected = parse(request(Operation::Own), Operation::Own).unwrap();
    let first = Session {
        id: expected.revision,
        account: Account {
            id: expected.account,
            nickname: Some(Nickname::parse("Daily Player").unwrap()),
        },
        created_at: NOW - 1000,
        authenticated_at: NOW - 1000,
        expires_at: NOW + 10,
    };
    assert!(proof(&first, &expected, NOW).is_ok());
    assert!(same(&first, &first, &expected, NOW).is_ok());
    for index in 0..11 {
        let mut bad = first.clone();
        match index {
            0 => bad.id = Uuid::nil(),
            1 => bad.account.id = Uuid::nil(),
            2 => bad.account.nickname = None,
            3 => bad.created_at -= 1,
            4 => bad.created_at = NOW + 1,
            5 => bad.authenticated_at = bad.created_at - 1,
            6 => bad.authenticated_at = NOW + 1,
            7 => bad.expires_at = NOW,
            8 => bad.id = Uuid::new_v4(),
            9 => bad.account.id = Uuid::new_v4(),
            _ => bad.expires_at += 1,
        }
        assert_eq!(
            same(&first, &bad, &expected, NOW),
            Err(Failure::Unauthorized)
        );
    }
    for field in [0, 1] {
        let mut bad = first.clone();
        if field == 0 {
            bad.created_at -= 1;
        } else {
            bad.authenticated_at += 1;
        }
        assert_eq!(
            same(&first, &bad, &expected, NOW),
            Err(Failure::Unauthorized)
        );
    }
    assert!(valid(&first, 0).is_err());
}
#[test]
fn receipt_projection_checks_original_identity_metadata_and_exact_retention() {
    let definition = PublishedDaily::new("2026-10-07", 1, RulesSnapshot::bundled()).unwrap();
    let receipt = DailyReceipt {
        id: Uuid::new_v4(),
        attempt: Uuid::new_v4(),
        metadata: definition.metadata().clone(),
        opened_safe: 216,
        mistakes: 1,
        replayed_elapsed_ms: 3200,
        verified_elapsed_ms: None,
        completed_at: NOW,
    };
    assert_eq!(
        completion(&receipt, &definition, NOW)
            .unwrap()
            .replayed_elapsed_ms,
        3200
    );
    for index in 0..8 {
        let mut bad = receipt.clone();
        match index {
            0 => bad.id = Uuid::nil(),
            1 => bad.attempt = Uuid::nil(),
            2 => bad.metadata.seed = "9".into(),
            3 => bad.opened_safe = 215,
            4 => bad.mistakes = RulesSnapshot::bundled().rules.max_commands_per_seat + 1,
            5 => bad.completed_at = 0,
            6 => bad.completed_at = NOW + 1,
            _ => bad.completed_at = NOW - 90 * 86400,
        };
        assert!(completion(&bad, &definition, NOW).is_err());
    }
    assert!(completion(&receipt, &definition, NOW + 90 * 86400 - 1).is_ok());
    assert!(completion(&receipt, &definition, NOW + 90 * 86400).is_err());
    for error in [
        ReplayError::Busy,
        ReplayError::Timeout,
        ReplayError::Unavailable,
    ] {
        assert_eq!(Failure::from(error), Failure::Unavailable);
    }
    assert_eq!(Failure::from(ReplayError::Incomplete), Failure::Invalid);
    for (error, expected) in [
        (DailyStoreError::Invalid, Failure::Invalid),
        (DailyStoreError::Unauthorized, Failure::Unauthorized),
        (DailyStoreError::Conflict, Failure::Conflict),
        (DailyStoreError::Expired, Failure::Expired),
        (DailyStoreError::Unavailable, Failure::Unavailable),
    ] {
        assert_eq!(Failure::from(error), expected);
    }
}
#[test]
fn configuration_is_bounded_and_strict() {
    assert_eq!(
        load_daily_config(|_| None).unwrap(),
        DailyConfig {
            authorities: 64,
            requests: 4
        }
    );
    assert_eq!(
        load_daily_config(|name| Some(
            if name == "LIAR_DAILY_REQUESTS" {
                "64"
            } else {
                "20000"
            }
            .into()
        ))
        .unwrap(),
        DailyConfig {
            authorities: 20000,
            requests: 64
        }
    );
    for bad in [
        "",
        "0",
        "-1",
        "+2",
        " 2",
        "2.0",
        "9999999999999999999999999",
        "20001",
    ] {
        assert!(load_daily_config(|_| Some(bad.into())).is_err());
    }
    assert!(
        load_daily_config(|name| (name == "LIAR_DAILY_REQUESTS").then(|| "65".into())).is_err()
    );
}
