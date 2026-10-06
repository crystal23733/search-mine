use liar_wasm::session::PracticeSession;
use serde_json::{Value, json};
use std::{env, fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    for (seed, difficulty) in [
        ("0", "easy"),
        ("42", "normal"),
        ("18446744073709551615", "hard"),
    ] {
        let mut session =
            PracticeSession::new(seed, difficulty).map_err(|_| "Fixture initialization failed")?;
        let initial = session.view();
        let cell = initial
            .own
            .cells
            .iter()
            .find(|c| c.state == liar_protocol::game::CellState::Closed)
            .ok_or("No closed cell")?
            .cell;
        let frame = |id: u32, seq: u32, action: Value| {
            json!({"v":1,"command_id":id,"client_seq":seq,"action":action}).to_string()
        };
        let flag = frame(1, 1, json!({"type":"flag","cell":cell}));
        let actions = vec![
            ("step", flag.clone(), 3000),
            ("step", flag, 3001),
            (
                "step",
                frame(2, 2, json!({"type":"open","cell":cell})),
                3002,
            ),
            (
                "step",
                frame(3, 3, json!({"type":"flag","cell":cell})),
                3003,
            ),
            (
                "step",
                frame(4, 4, json!({"type":"open","cell":cell})),
                3004,
            ),
            ("step", "{}".into(), 4000),
            (
                "step",
                r#"{"v":99,"command_id":5,"client_seq":5,"action":{"type":"attack"}}"#.into(),
                4000,
            ),
            ("step", frame(5, 5, json!({"type":"attack"})), 6000),
            ("step", frame(6, 6, json!({"type":"accuse","cell":0})), 6001),
            ("advance", String::new(), 6500),
            ("advance", String::new(), 10000),
            ("advance", String::new(), 7000),
            ("advance", String::new(), 243000),
        ];
        let mut events = Vec::new();
        for (kind, input, time_ms) in actions {
            let result = if kind == "step" {
                session
                    .step(&input, time_ms)
                    .map(|v| serde_json::to_value(v).unwrap())
            } else {
                session
                    .advance(time_ms)
                    .map(|v| serde_json::to_value(v).unwrap())
            };
            let expected = match result {
                Ok(value) => json!({"ok":value}),
                Err(error) => json!({"error":error}),
            };
            events.push(json!({"kind":kind,"input":input,"time_ms":time_ms,"expected":expected}));
        }
        cases.push(json!({"seed":seed,"difficulty":difficulty,"initial":initial,"events":events}));
    }
    let source = serde_json::to_string_pretty(&json!({"v":1,"cases":cases}))? + "\n";
    let target =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/native-wasm.json");
    if env::args().any(|arg| arg == "--check") {
        if fs::read_to_string(&target)? != source {
            return Err("Native fixture is stale; run pnpm fixtures:generate".into());
        }
    } else {
        fs::create_dir_all(target.parent().ok_or("Invalid path")?)?;
        fs::write(target, source)?;
    }
    Ok(())
}
