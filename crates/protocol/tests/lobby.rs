use liar_protocol::lobby::*;
use serde_json::json;
use uuid::Uuid;

#[test]
fn versioned_lobby_intents_keep_identity_and_large_generations_without_numeric_loss() {
    let id = "abcdef12-3456-4789-9012-345678901234".to_string();
    let commands = [
        json!({"type":"status"}),
        json!({"type":"queue_join","difficulty":"hard"}),
        json!({"type":"room_create"}),
        json!({"type":"room_join","code":"ABCD2345"}),
        json!({"type":"ready","room_id":id,"ready":true}),
        json!({"type":"cancel","identity":{"kind":"queue","queue_id":id}}),
        json!({"type":"cancel","identity":{"kind":"room","room_id":id}}),
        json!({"type":"cancel","identity":{"kind":"preparing","entity":id,"generation":u64::MAX.to_string()}}),
    ];
    for command in commands {
        let raw = json!({"v":1,"command":command});
        let accepted = decode_lobby(&serde_json::to_vec(&raw).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(accepted).unwrap(), raw);
    }
}
#[test]
fn lobby_decoder_rejects_forged_authority_duplicates_noncanonical_ids_and_unbounded_frames() {
    let id = "abcdef12-3456-4789-9012-345678901234".to_string();
    let commands = [
        json!({"type":"status","account":id}),
        json!({"type":"status","seat":0}),
        json!({"type":"queue_join","difficulty":"expert"}),
        json!({"type":"ready","room_id":Uuid::nil().to_string(),"ready":true}),
        json!({"type":"ready","room_id":id.to_uppercase(),"ready":true}),
        json!({"type":"cancel","identity":{"kind":"preparing","entity":id,"generation":"01"}}),
        json!({"type":"cancel","identity":{"kind":"preparing","entity":id,"generation":"0"}}),
        json!({"type":"cancel","identity":{"kind":"preparing","entity":id,"generation":"18446744073709551616"}}),
        json!({"type":"cancel","identity":{"kind":"preparing","entity":id,"generation":1}}),
    ];
    for command in commands {
        assert_eq!(
            decode_lobby(&serde_json::to_vec(&json!({"v":1,"command":command})).unwrap()),
            Err(LobbyErrorCode::Malformed)
        );
    }
    for raw in [
        r#"{"v":1,"v":1,"command":{"type":"status"}}"#,
        r#"{"v":1,"command":{"type":"status","type":"status"}}"#,
        r#"{"v":1,"command":{"type":"status"},"seed":46}"#,
    ] {
        assert_eq!(decode_lobby(raw.as_bytes()), Err(LobbyErrorCode::Malformed));
    }
    assert_eq!(
        decode_lobby(&vec![b' '; MAX_LOBBY_BYTES + 1]),
        Err(LobbyErrorCode::Malformed)
    );
    assert_eq!(
        decode_lobby(br#"{"v":2,"command":{"type":"status"}}"#),
        Err(LobbyErrorCode::UnsupportedVersion)
    );
}
#[test]
fn public_lobby_state_has_only_self_placement_and_string_cancellation_identity() {
    let id = "abcdef12-3456-4789-9012-345678901234".to_string();
    let response = LobbyResponse {
        v: 1,
        server_time_ms: 1000,
        state: LobbyState::Preparing {
            identity: LobbyCancellation::Preparing {
                entity: id.clone(),
                generation: u64::MAX.to_string(),
            },
            opponent: LobbyOpponent::Human,
        },
    };
    assert_eq!(
        serde_json::to_value(response).unwrap(),
        json!({"v":1,"server_time_ms":1000,"state":{"type":"preparing","identity":{"kind":"preparing","entity":id,"generation":u64::MAX.to_string()},"opponent":"human"}})
    );
    let declarations = liar_protocol::public_types();
    assert!(declarations.contains("export type LobbyInput"));
    assert!(declarations.contains("generation: string"));
}
