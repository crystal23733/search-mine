//! Offline inspection must complete without any DB configuration or personal output.
use liar_server::auth::{AuthClock, SystemAuthClock};
use std::{
    io::Write,
    process::{Command, Output, Stdio},
};
fn inspect(args: &[&str], input: &[u8]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_recovery"));
    cmd.env_clear()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for key in [
        "PATH",
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(value) = std::env::var_os(key) {
            cmd.env(key, value);
        }
    }
    let mut child = cmd.spawn().unwrap();
    if let Err(error) = child.stdin.take().unwrap().write_all(input) {
        assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
    }
    child.wait_with_output().unwrap()
}
#[test]
fn authenticated_snapshot_can_be_inspected_offline_without_returning_personal_ids() {
    let now = SystemAuthClock.now();
    let source = "11111111-1111-4111-8111-111111111111";
    let private = "22222222-2222-4222-8222-222222222222";
    let input=serde_json::json!({"version":1,"source_id":source,"captured_at":now,"entries":[{"kind":"account","id":private,"deleted_at":now-10,"expires_at":now-10+2419200}]}).to_string();
    let output = inspect(&["inspect-snapshot"], input.as_bytes());
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        serde_json::json!({"source_id":source,"captured_at":now,"entries":1})
    );
    assert!(!String::from_utf8(output.stdout).unwrap().contains(private));
}
#[test]
fn offline_inspection_rejects_malformed_or_oversized_input_with_only_fixed_failure() {
    for (args, input) in [
        (vec!["inspect-snapshot"], b"sensitive sentinel".to_vec()),
        (vec!["inspect-snapshot"], vec![b' '; 128 * 1024 + 1]),
        (vec!["inspect-snapshot", "extra"], b"{}".to_vec()),
    ] {
        let output = inspect(&args, &input);
        assert!(!output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            serde_json::json!({"status":"failed","code":"recovery_failed"})
        );
    }
}
