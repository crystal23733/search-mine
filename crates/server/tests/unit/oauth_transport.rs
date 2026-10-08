use super::*;
#[tokio::test]
async fn only_fixed_refresh_grant_http400_invalid_grant_can_revoke_identity() {
    let response = |status, body: &str| {
        reqwest::Response::from(
            axum::http::Response::builder()
                .status(status)
                .body(body.to_string())
                .unwrap(),
        )
    };
    assert!(matches!(
        bounded_grant_response(response(400, r#"{"error":"invalid_grant"}"#), 65536, true).await,
        Err(AuthError::CredentialRevoked)
    ));
    for (status, body, refresh) in [
        (400, r#"{"error":"invalid_client"}"#, true),
        (400, r#"{"error":"invalid_grant"}"#, false),
        (500, r#"{"error":"invalid_grant"}"#, true),
        (400, "malformed", true),
    ] {
        assert!(matches!(
            bounded_grant_response(response(status, body), 65536, refresh).await,
            Err(AuthError::Unavailable)
        ));
    }
    assert!(
        bounded_grant_response(response(400, &"x".repeat(65537)), 65536, true)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn upstream_response_limits_and_http_failures_never_return_identity_bytes() {
    let response = |status: u16, body: Vec<u8>| {
        reqwest::Response::from(
            axum::http::Response::builder()
                .status(status)
                .body(body)
                .unwrap(),
        )
    };
    assert_eq!(
        bounded_response(response(200, b"public-jwks".to_vec()), 32)
            .await
            .unwrap()
            .as_slice(),
        b"public-jwks"
    );
    for status in [302, 400, 401, 500] {
        assert!(
            bounded_response(response(status, b"provider-private-error".to_vec()), 64)
                .await
                .is_err()
        );
    }
    assert!(
        bounded_response(response(200, vec![0; 65537]), 65536)
            .await
            .is_err()
    );
    let transport = HttpsOAuthTransport::new().unwrap();
    assert!(
        transport
            .request(UpstreamRequest {
                endpoint: "https://attacker.example",
                form: vec![],
                bearer: None,
                limit: 64
            })
            .await
            .is_err()
    );
}
