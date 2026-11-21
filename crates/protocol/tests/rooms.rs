use liar_protocol::rooms::{RoomIntent, RoomWireError, decode_room_request};
use serde_json::json;
const ROOM: &str = "00000000-0000-0000-0000-000000000001";
const REQUEST: &str = "00000000-0000-0000-0000-000000000002";

#[test]
fn v2_requires_closed_inputs_and_revision_when_changing_a_room() {
    let input = json!({"v":2,"intent":{"type":"ready","request_id":REQUEST,"room_id":ROOM,"revision":"2","ready":true}});
    let decode = |value: &serde_json::Value| decode_room_request(value.to_string().as_bytes());
    assert!(matches!(
        decode(&input).unwrap().intent,
        RoomIntent::Ready { ready: true, .. }
    ));
    for (field, value) in [
        ("revision", json!("02")),
        ("revision", json!(2)),
        ("room_id", json!("invalid")),
        ("target", json!(0)),
    ] {
        let mut bad = input.clone();
        bad["intent"][field] = value;
        assert_eq!(decode(&bad), Err(RoomWireError::Malformed));
    }
    let mut bad_version = input.clone();
    bad_version["v"] = json!(1);
    assert_eq!(decode(&bad_version), Err(RoomWireError::UnsupportedVersion));
    assert_eq!(
        decode_room_request(&[b' '; 2049]),
        Err(RoomWireError::Malformed)
    );
}

#[test]
fn room_options_entrances_and_list_cursor_have_fixed_shapes() {
    let decode =
        |intent| decode_room_request(json!({"v":2,"intent":intent}).to_string().as_bytes());
    let options = json!({"title":"Mines 2","visibility":"invite","capacity":3});
    assert!(decode(json!({"type":"create","request_id":REQUEST,"options":options})).is_ok());
    assert_eq!(
        decode(
            json!({"type":"create","request_id":REQUEST,"options":{"title":"Room","visibility":"public","capacity":5}})
        ),
        Err(RoomWireError::Malformed)
    );
    assert!(
        decode(
            json!({"type":"join","request_id":REQUEST,"entrance":{"kind":"public","room_id":ROOM}})
        )
        .is_ok()
    );
    assert!(decode(json!({"type":"join","request_id":REQUEST,"entrance":{"kind":"invite","code":"ABCDEFG2"}})).is_ok());
    for code in ["abcdefg2", "ABCDEFG0", "ABCDEFG", "OBCDEFG2"] {
        assert_eq!(
            decode(
                json!({"type":"join","request_id":REQUEST,"entrance":{"kind":"invite","code":code}})
            ),
            Err(RoomWireError::Malformed)
        );
    }
    assert!(decode(json!({"type":"list","cursor":{"accepting":true,"room_id":ROOM}})).is_ok());
    assert_eq!(
        decode(json!({"type":"list","cursor":{"accepting":true,"room_id":ROOM,"secret":"extra"}})),
        Err(RoomWireError::Malformed)
    );
    assert!(decode(json!({"type":"configure","request_id":REQUEST,"room_id":ROOM,"revision":"2","options":options})).is_ok());
    assert!(
        decode(json!({"type":"kick","request_id":REQUEST,"room_id":ROOM,"revision":"2","seat":3}))
            .is_ok()
    );
    assert_eq!(
        decode(json!({"type":"kick","request_id":REQUEST,"room_id":ROOM,"revision":"2","seat":4})),
        Err(RoomWireError::Malformed)
    );
    assert!(
        decode(json!({"type":"start","request_id":REQUEST,"room_id":ROOM,"revision":"2"})).is_ok()
    );
    assert!(
        decode(json!({"type":"leave","request_id":REQUEST,"room_id":ROOM,"revision":"2"})).is_ok()
    );
    assert!(decode(json!({"type":"status"})).is_ok());
}
