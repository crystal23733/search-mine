use super::result_owner::{Server, server_command, wait_ready};
use super::*;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Write},
    ops::{Deref, DerefMut},
    path::PathBuf,
    process::Stdio,
    sync::Mutex,
    thread::JoinHandle,
    time::Instant,
};

pub(super) struct ObservedServer {
    server: Server,
    pub log: PathBuf,
    reader: Option<JoinHandle<std::io::Result<()>>>,
    started: Instant,
    states: Mutex<VecDeque<Value>>,
}
impl Deref for ObservedServer {
    type Target = Server;
    fn deref(&self) -> &Server {
        &self.server
    }
}
impl DerefMut for ObservedServer {
    fn deref_mut(&mut self) -> &mut Server {
        &mut self.server
    }
}
impl ObservedServer {
    pub async fn spawn(pool: &sqlx::PgPool) -> (Self, String) {
        let (mut command, origin) = server_command(pool).await;
        std::fs::create_dir_all(".tmp/admission-main").unwrap();
        let log = PathBuf::from(format!(
            ".tmp/admission-main/{}-{}.jsonl",
            std::process::id(),
            Uuid::new_v4()
        ));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&log)
            .unwrap();
        command.stderr(Stdio::piped());
        let started = Instant::now();
        let mut server = Server(command.spawn().unwrap());
        let stderr = server.0.stderr.take().unwrap();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                let record = match line {
                    Ok(line) => record(&line, elapsed),
                    Err(error) => {
                        writeln!(
                            file,
                            "{}",
                            json!({"component":"observer","code":"stderr_read_failed","observer_elapsed_ms":elapsed})
                        )?;
                        file.flush()?;
                        return Err(error);
                    }
                };
                writeln!(file, "{record}")?;
                file.flush()?;
            }
            Ok(())
        });
        (
            Self {
                server,
                log,
                reader: Some(reader),
                started,
                states: Mutex::new(VecDeque::new()),
            },
            origin,
        )
    }
    pub fn diagnostics(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_else(|_| "diagnostic_read_failed".to_owned())
    }
    pub fn cached(&self) -> bool {
        self.diagnostics().lines().any(|line| {
            serde_json::from_str::<Value>(line)
                .is_ok_and(|v| v["component"] == "board_pool" && v["code"] == "ready")
        })
    }
    pub fn observe(&self, reply: &Value) {
        let kind = reply["state"]["type"].as_str().unwrap_or("unknown");
        let kind = match kind {
            "idle" | "queued" | "room" | "preparing" | "matched" | "failed" => kind,
            _ => "unknown",
        };
        if let Ok(mut states) = self.states.lock() {
            if states.len() == 16 {
                states.pop_front();
            }
            states.push_back(json!({"state":kind,"server_time_ms":reply["server_time_ms"].as_u64(),"observer_elapsed_ms":self.started.elapsed().as_millis()}));
        }
    }
}
impl Drop for ObservedServer {
    fn drop(&mut self) {
        let _ = self.server.0.kill();
        let _ = self.server.0.wait();
        let joined = self.reader.take().map(|r| r.join());
        if !matches!(joined, Some(Ok(Ok(())))) {
            assert!(std::thread::panicking(), "diagnostic_reader_failed");
            eprintln!("diagnostic_reader_failed");
        }
        if std::thread::panicking() {
            let states = self.states.lock().map(|s| json!(*s)).unwrap_or(Value::Null);
            eprintln!(
                "Actual main diagnostics ({}): states={states}\n{}",
                self.log.display(),
                self.diagnostics()
            );
        }
    }
}
fn record(line: &str, elapsed: u64) -> Value {
    let mut output =
        json!({"component":"observer","code":"non_json_stderr","observer_elapsed_ms":elapsed});
    let Ok(input) = serde_json::from_str::<Value>(line) else {
        return output;
    };
    let Some(component) = input["component"].as_str() else {
        return output;
    };
    if !matches!(
        component,
        "board_pool" | "admission" | "event_retention" | "metric_retention" | "product_metrics"
    ) {
        return output;
    }
    let Some(code) = input["code"].as_str() else {
        return output;
    };
    if !matches!(
        code,
        "started"
            | "ready"
            | "complete"
            | "unavailable"
            | "invalid_clock"
            | "incomplete"
            | "capacity"
            | "expired"
            | "conflict"
            | "invalid"
            | "unauthorized"
            | "malformed"
            | "rate_limited"
            | "not_matched"
            | "wrong_match"
            | "invalid_epoch"
            | "unsupported_version"
            | "invalid_cell"
    ) {
        return output;
    }
    output["component"] = json!(component);
    output["code"] = json!(code);
    if let Some(stage) = input["stage"].as_str()
        && matches!(
            stage,
            "board_take" | "prepare" | "prepare_join" | "register"
        )
    {
        output["stage"] = json!(stage);
    }
    if let Some(slot) = input["slot"].as_str()
        && matches!(slot, "current" | "rotating" | "cycle")
    {
        output["slot"] = json!(slot);
    }
    for name in ["deleted", "batches"] {
        if let Some(n) = input[name].as_u64() {
            output[name] = json!(n);
        }
    }
    if let Some(more) = input["has_more"].as_bool() {
        output["has_more"] = json!(more);
    }
    output
}

#[tokio::test]
#[ignore = "requires actual PostgreSQL; executed in database CI"]
async fn real_child_restarts_and_concurrent_owners_never_overwrite_an_earlier_log() {
    let pool = auth_pool().await;
    let (mut first, origin) = ObservedServer::spawn(&pool).await;
    wait_ready(&mut first, &origin).await;
    let first_path = first.log.clone();
    drop(first);
    let original = std::fs::read(&first_path).unwrap();
    let (mut next, origin) = ObservedServer::spawn(&pool).await;
    wait_ready(&mut next, &origin).await;
    assert_ne!(
        first_path, next.log,
        "Restarted children must preserve independent diagnostics"
    );
    assert_eq!(std::fs::read(&first_path).unwrap(), original);
    let next_path = next.log.clone();
    let (mut concurrent, _) = ObservedServer::spawn(&pool).await;
    super::result_owner::wait_failed(&mut concurrent).await;
    assert_ne!(concurrent.log, next_path);
    assert_ne!(concurrent.log, first_path);
    assert_eq!(std::fs::read(&first_path).unwrap(), original);
    drop(concurrent);
    next.observe(&json!({"state":{"type":"matched","match_id":"not-logged"},"server_time_ms":1}));
    drop(next);
    let lines = std::fs::read_to_string(next_path).unwrap();
    assert!(!lines.is_empty());
    assert!(lines.lines().all(|line| {
        serde_json::from_str::<Value>(line).unwrap()["observer_elapsed_ms"].is_u64()
    }));
    close_auth_pool(pool).await;
}

#[test]
fn diagnostics_keep_only_closed_stages_and_operational_counts() {
    let input = json!({"component":"admission","stage":"register","code":"ready","account_id":"private","seed":"private","sql":"private","token":"private"});
    let safe = record(&input.to_string(), 123);
    assert_eq!(
        safe,
        json!({"component":"admission","stage":"register","code":"ready","observer_elapsed_ms":123})
    );
    for line in [
        "postgres://private",
        r#"{"component":"private","code":"ready"}"#,
        r#"{"component":"admission","code":"private"}"#,
    ] {
        assert_eq!(
            record(line, 124),
            json!({"component":"observer","code":"non_json_stderr","observer_elapsed_ms":124})
        );
    }
}
