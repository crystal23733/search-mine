use liar_core::{
    board::ObservedCell,
    daily::DailyPuzzle,
    game::{Action, Command, Seat},
    solver::{NoGuessSolver, SolverBudget},
};
use liar_wasm::daily::DailySession;
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
    // Generated full public snapshots are compact; review the explicit replay inputs above.
    let source = serde_json::to_string(&json!({"v":1,"cases":cases,"daily":daily_case()?}))? + "\n";
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
fn daily_case() -> Result<Value, Box<dyn std::error::Error>> {
    let date = "2026-10-07";
    let mut session = DailySession::new(date, 1).map_err(|_| "Daily initialization failed")?;
    let mut puzzle = DailyPuzzle::new(date, 1).map_err(|_| "Daily puzzle failed")?;
    let initial = session.view();
    let mut events = Vec::new();
    let mut serial = 0_u32;
    while !puzzle.complete() {
        let view = puzzle.projection();
        let cell = NoGuessSolver::deduce(&view.own.cells, SolverBudget::default())
            .map_err(|_| "Public solve failed")?
            .safe
            .into_iter()
            .find(|&cell| view.own.cells.cell(cell) == Some(ObservedCell::Unknown))
            .ok_or("No safe public progress")?;
        serial += 1;
        let time_ms = 3000 + serial * 50;
        let input = json!({"v":1,"command_id":serial,"client_seq":serial,"action":{"type":"open","cell":cell.0}}).to_string();
        puzzle.apply(Command {
            id: serial.into(),
            seq: serial.into(),
            epoch: 1,
            seat: Seat::One,
            received_at: u64::from(time_ms),
            action: Action::Open(cell),
        });
        let step = session
            .step(&input, time_ms)
            .map_err(|_| "Daily replay failed")?;
        events.push(json!({"input":input,"time_ms":time_ms,"expected":step}));
    }
    let replay = session.export_replay();
    let restored = DailySession::from_replay(&serde_json::to_string(&replay)?)
        .map_err(|_| "Daily restore failed")?;
    if restored.view() != session.view() || !restored.view().completed {
        return Err("Daily roundtrip mismatch".into());
    }
    Ok(json!({"date":date,"seed_version":1,"initial":initial,"events":events,"replay":replay}))
}
