use crate::{
    app::{AppState, ExecuteRequest, ExecuteResponse},
    piston::{PistonFile, PistonReponse, PistonRequest},
};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use std::{env::var, sync::Arc, time::Duration};

mod app;
mod piston;

#[tokio::main]
async fn main() {
    // Get key and init state
    let (api_key, piston_url, language, version) = get_env_vars();

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("http client failed to start");

    let state = AppState::new(api_key, http, piston_url, language, version);

    // Bind listener to port 8080
    let address = "127.0.0.1:8080";
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    println!("Listening on 8080");

    // Start server with axum gateway
    axum::serve(listener, build_router(state)).await.unwrap();
}

// Get all environment variables
fn get_env_vars() -> (String, String, String, String) {
    let api_key = var("GATEWAY_API_KEY").unwrap_or_else(|_| "dev_key".into());
    let piston_url = var("PISTON_URL").unwrap_or_else(|_| "http://localhost:2000".into());
    let language = var("PISTON_LANGUAGE").unwrap_or_else(|_| "javascript".into());
    let version = var("PISTON_VERSION").unwrap_or_else(|_| "*".into());

    (api_key, piston_url, language, version)
}

// Build router
fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/execute", post(execute))
        .with_state(state)
}

// This endpoints exists just to check that the server works
async fn health() -> &'static str {
    "ok"
}

// Proxy request to piston instance then respond with piston response
async fn execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    // Auth before anythng touches piston
    let provided = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided != state.api_key.as_str() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let piston_req = PistonRequest {
        language: state.language.as_str().to_string(),
        version: state.version.as_str().to_string(),
        files: vec![PistonFile { content: req.code }],
        stdin: req.stdin,
        run_timeout: 3000,
        run_memory_limit: 128 * 1024 * 1024, // 128 MB per run
    };

    let url = format!("{}/api/v2/execute", state.piston_url);

    let resp = state
        .http
        .post(&url)
        .json(&piston_req)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?; // Piston unreachable/timed out

    if !resp.status().is_success() {
        return Err(StatusCode::BAD_GATEWAY);
    }

    let piston: PistonReponse = resp.json().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    // Just a stubout for now will forward to piston instance later

    Ok(Json(ExecuteResponse {
        stdout: piston.run.stdout,
        stderr: piston.run.stderr,
        exit_code: piston.run.code.unwrap_or(-1),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt; // for `oneshot`

    fn test_state(piston_url: String) -> AppState {
        AppState {
            api_key: Arc::new("secret".into()),
            http: reqwest::Client::new(),
            piston_url: Arc::new(piston_url),
            language: Arc::new("javascript".into()),
            version: Arc::new("*".into()),
        }
    }

    fn post_execute(key: Option<&str>) -> Request<Body> {
        let mut b = Request::builder()
            .method("POST")
            .uri("/execute")
            .header("content-type", "application/json");
        if let Some(k) = key {
            b = b.header("x-api-key", k);
        }
        b.body(Body::from(r#"{"code":"console.log(1)","stdin":""}"#))
            .unwrap()
    }

    #[tokio::test]
    async fn wrong_key_is_rejected() {
        // bogus piston url is fine: auth fails before we ever call it
        let app = build_router(test_state("http://127.0.0.1:1".into()));
        let res = app.oneshot(post_execute(Some("nope"))).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn missing_key_is_rejected() {
        let app = build_router(test_state("http://127.0.0.1:1".into()));
        let res = app.oneshot(post_execute(None)).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn correct_key_maps_piston_output() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        // stand up a fake Piston that returns a canned result
        let piston = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/execute"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "run": { "stdout": "1\n", "stderr": "", "code": 0 }
            })))
            .mount(&piston)
            .await;

        let app = build_router(test_state(piston.uri()));
        let res = app.oneshot(post_execute(Some("secret"))).await.unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["stdout"], "1\n");
        assert_eq!(v["exit_code"], 0);
    }
}
