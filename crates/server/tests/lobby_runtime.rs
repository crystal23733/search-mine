use reqwest::StatusCode;
use std::{
    process::{Child, Command, Stdio},
    time::Duration,
};
struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn configured(address: &str, auth: bool) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_liar-server"));
    command
        .env_clear()
        .env("LIAR_BIND", address)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    for name in [
        "SystemRoot",
        "WINDIR",
        "PATH",
        "TEMP",
        "TMP",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    if auth {
        command.env("DATABASE_URL","postgres://fixture:fixture@127.0.0.1:1/fixture")
            .env("LIAR_PUBLIC_ORIGIN","https://game.example")
            .env("LIAR_AUTH_CSRF_KEY","AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE")
            .env("LIAR_AUTH_DIGEST_KEYS",r#"{"current":1,"keys":[{"version":1,"key":"AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI"}]}"#)
            .env("LIAR_AUTH_VAULT_KEYS",r#"{"current":1,"keys":[{"version":1,"key":"AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM"}]}"#);
    }
    command
}
#[tokio::test]
async fn standalone_main_exposes_disabled_lobby_without_authentication() {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap().to_string();
    drop(reserved);
    let mut server = Server(configured(&address, false).spawn().unwrap());
    let origin = format!("http://{address}");
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            assert!(server.0.try_wait().unwrap().is_none());
            if client
                .get(format!("{origin}/health/live"))
                .send()
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let result = client
        .post(format!("{origin}/api/v1/lobby"))
        .json(&serde_json::json!({"v":1,"command":{"type":"status"}}))
        .send()
        .await
        .unwrap();
    assert_eq!(result.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(result.headers()["cache-control"], "no-store");
}
#[tokio::test]
async fn standalone_main_fails_closed_when_online_database_is_unreachable() {
    let mut server = Server(configured("127.0.0.1:0", true).spawn().unwrap());
    let status = tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if let Some(status) = server.0.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(!status.success());
}
#[tokio::test]
async fn standalone_main_rejects_invalid_lobby_configuration_before_serving() {
    let mut command = configured("127.0.0.1:0", true);
    command.env("LIAR_LOBBY_CAPACITY", "0");
    let mut server = Server(command.spawn().unwrap());
    let status = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(status) = server.0.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(!status.success());
}

#[cfg(unix)]
async fn assert_normal_signal_exit(signal: &str) {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap().to_string();
    drop(reserved);
    let mut server = Server(configured(&address, false).spawn().unwrap());
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            assert!(server.0.try_wait().unwrap().is_none());
            if client
                .get(format!("http://{address}/health/live"))
                .send()
                .await
                .is_ok_and(|response| response.status() == StatusCode::OK)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("The owned child must become live");
    let sent = Command::new("/bin/kill")
        .arg(signal)
        .arg(server.0.id().to_string())
        .status()
        .unwrap();
    assert!(sent.success());
    let status = tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if let Some(status) = server.0.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("An idle server must shut down normally after the signal");
    assert!(
        status.success(),
        "{signal} must take the normal server exit path: {status:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn standalone_main_terminates_cleanly_on_sigterm() {
    assert_normal_signal_exit("-TERM").await;
}

#[cfg(unix)]
#[tokio::test]
async fn standalone_main_terminates_cleanly_on_sigint() {
    assert_normal_signal_exit("-INT").await;
}
