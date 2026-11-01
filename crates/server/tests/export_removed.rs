use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
#[tokio::test]
async fn unconfigured_composition_does_not_register_personal_file_endpoints() {
    for path in [
        "/api/v1/me/export",
        "/api/v1/me/games/export",
        "/api/v1/me/games/replay",
    ] {
        let response = liar_server::app(None)
            .oneshot(Request::post(path).body(Body::from("{}")).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert!(body.is_empty());
    }
}
