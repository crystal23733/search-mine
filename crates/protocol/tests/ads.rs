use liar_protocol::{ads::*, game::PublicError};
use serde_json::{Value, json};

fn menu() -> Value {
    json!({"v":1,"opportunity":{
        "placement":"menu","screen":"menu","online":true,"advertising_allowed":true,
        "sdk_ready":true,"exclusive_slot":true,"counters":null,"result":null
    }})
}
fn evaluate(value: Value) -> Result<String, PublicError> {
    plan_ad_json(&value.to_string())
}

#[test]
fn closed_contract_serializes_native_plan_and_version_errors() {
    assert_eq!(evaluate(menu()).unwrap(), r#"{"type":"menu"}"#);
    let mut input = menu();
    input["v"] = json!(2);
    assert_eq!(evaluate(input), Err(PublicError::UnsupportedVersion));
    assert!(liar_protocol::public_types().contains("export type AdPlan ="));
}

#[test]
fn unknown_duplicate_missing_and_unbounded_input_are_not_plans() {
    let mut extra = menu();
    extra["account_id"] = json!("private");
    let mut nested = menu();
    nested["opportunity"]["seed"] = json!("secret");
    let mut missing = menu();
    missing["opportunity"]
        .as_object_mut()
        .unwrap()
        .remove("result");
    let mut unknown = menu();
    unknown["opportunity"]["screen"] = json!("game");
    let mut wrong = menu();
    wrong["opportunity"]["online"] = json!("true");
    for value in [
        extra,
        nested,
        missing,
        unknown,
        wrong,
        json!(null),
        json!([]),
    ] {
        assert_eq!(evaluate(value), Err(PublicError::Malformed));
    }
    let source = menu().to_string();
    assert_eq!(
        plan_ad_json(&source.replacen("\"v\":1", "\"v\":1,\"v\":1", 1)),
        Err(PublicError::Malformed)
    );
    assert_eq!(
        plan_ad_json(&format!("{source}{}", " ".repeat(MAX_AD_INPUT_BYTES))),
        Err(PublicError::Malformed)
    );
    assert_eq!(
        plan_ad_json(&format!(
            "{source}{}",
            " ".repeat(MAX_AD_INPUT_BYTES - source.len())
        ))
        .unwrap(),
        r#"{"type":"menu"}"#
    );
}

#[test]
fn authenticated_counter_cannot_be_omitted_or_encoded_with_unknown_or_invalid_numbers() {
    let mut input = menu();
    input["opportunity"]["placement"] = json!("result");
    input["opportunity"]["screen"] = json!("result");
    input["opportunity"]["result"] = json!({"completion":"daily","already_requested":false});
    input["opportunity"]["counters"] =
        json!({"identity":"account","browser":{"completed":3,"last_requested":null}});
    assert_eq!(evaluate(input.clone()), Err(PublicError::Malformed));
    input["opportunity"]["counters"]["account"] = json!({"completed":3,"last_requested":null});
    assert!(evaluate(input.clone()).unwrap().contains("reserve_result"));
    for number in [json!(-1), json!(3.5), json!(4_294_967_296_u64), json!("3")] {
        input["opportunity"]["counters"]["account"]["completed"] = number;
        assert_eq!(evaluate(input.clone()), Err(PublicError::Malformed));
    }
    input["opportunity"]["counters"]["account"]["completed"] = json!(3);
    input["opportunity"]["counters"]["extra"] = json!(true);
    assert_eq!(evaluate(input), Err(PublicError::Malformed));
}
