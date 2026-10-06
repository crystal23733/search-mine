use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use liar_server::app;
use tower::ServiceExt;

#[tokio::test]
async fn liveness_works_without_database_and_discloses_no_configuration() {
    let response = app(None)
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"status":"ok"})
    );
}

#[tokio::test]
async fn readiness_without_database_is_unavailable() {
    let response = app(None)
        .oneshot(
            Request::builder()
                .uri("/health/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
