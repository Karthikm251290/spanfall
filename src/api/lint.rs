use axum::Json;
use serde::Serialize;

/// `GET /api/lint` (§6). Spec 03 owns the body shape -- this is a stub so the route exists and
/// M0's surface is complete; the client can wire against it before M0's lint rules land.
#[derive(Serialize)]
pub struct LintResponse {
    findings: Vec<serde_json::Value>,
}

pub async fn get_lint() -> Json<LintResponse> {
    Json(LintResponse { findings: Vec::new() })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::store::Store;

    #[tokio::test]
    async fn returns_an_empty_findings_list() {
        let state = super::super::test_state(Store::new(10_000_000));
        let app = super::super::router(state);

        let response = app.oneshot(Request::builder().uri("/api/lint").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["findings"], serde_json::json!([]));
    }
}
