use reqwest::StatusCode;
use std::{
    io::Read,
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
    for name in ["SystemRoot", "WINDIR", "PATH", "TEMP", "TMP"] {
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
async fn standalone_main_exposes_disabled_lobby_and_shares_auth_bootstrap_csrf_gate() {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    for auth in [false, true] {
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reserved.local_addr().unwrap().to_string();
        drop(reserved);
        let mut server = Server(configured(&address, auth).spawn().unwrap());
        let origin = format!("http://{address}");
        tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                if let Some(status) = server.0.try_wait().unwrap() {
                    let mut diagnostic = String::new();
                    server
                        .0
                        .stderr
                        .as_mut()
                        .unwrap()
                        .read_to_string(&mut diagnostic)
                        .unwrap();
                    panic!("Standalone fixture auth={auth} exited {status}: {diagnostic}");
                }
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
        assert_eq!(
            result.status(),
            if auth {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            }
        );
        assert_eq!(result.headers()["cache-control"], "no-store");
        if auth {
            let bootstrap = client
                .get(format!("{origin}/api/v1/auth/bootstrap"))
                .send()
                .await
                .unwrap();
            assert_eq!(bootstrap.status(), StatusCode::OK);
            let browser = bootstrap
                .headers()
                .get_all("set-cookie")
                .iter()
                .filter_map(|h| h.to_str().ok())
                .find(|h| h.starts_with("__Host-liar_browser="))
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_string();
            let body: serde_json::Value = bootstrap.json().await.unwrap();
            let result = client
                .post(format!("{origin}/api/v1/lobby"))
                .header("origin", "https://game.example")
                .header("cookie", browser)
                .header("x-liar-csrf", body["csrf"].as_str().unwrap())
                .json(&serde_json::json!({"v":1,"command":{"type":"status"}}))
                .send()
                .await
                .unwrap();
            assert_eq!(
                result.status(),
                StatusCode::UNAUTHORIZED,
                "Auth bootstrap CSRF must pass the shared lobby security gate before missing-session rejection"
            );
        }
    }
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
