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
use std::{env::var, time::Duration};

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
    use axum::response::IntoResponse;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};

    // Spawns a router on an ephemeral port and returns its base URL.
    async fn spawn(router: Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        format!("http://{addr}")
    }

    // Stand-in for a Piston instance. Replies with `status` and `body` to any
    // POST /api/v2/execute, and records the request bodies it received.
    async fn spawn_piston(status: StatusCode, body: Value) -> (String, Arc<Mutex<Vec<Value>>>) {
        let seen: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
        let recorder = seen.clone();

        let router = Router::new().route(
            "/api/v2/execute",
            post(move |Json(req): Json<Value>| {
                let recorder = recorder.clone();
                let body = body.clone();
                async move {
                    recorder.lock().unwrap().push(req);
                    (status, Json(body)).into_response()
                }
            }),
        );

        (spawn(router).await, seen)
    }

    // Gateway wired to the given Piston URL, with a known API key.
    fn state_for(piston_url: &str) -> AppState {
        AppState::new(
            "test_key".into(),
            reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            piston_url.into(),
            "javascript".into(),
            "*".into(),
        )
    }

    fn ok_run() -> Value {
        json!({ "run": { "stdout": "hi\n", "stderr": "", "code": 0 } })
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let url = spawn(build_router(state_for("http://unused"))).await;

        let resp = reqwest::get(format!("{url}/health")).await.unwrap();

        assert_eq!(resp.status(), 200);
        assert_eq!(resp.text().await.unwrap(), "ok");
    }

    #[tokio::test]
    async fn execute_forwards_piston_output() {
        let (piston_url, seen) = spawn_piston(StatusCode::OK, ok_run()).await;
        let url = spawn(build_router(state_for(&piston_url))).await;

        let resp = reqwest::Client::new()
            .post(format!("{url}/execute"))
            .header("x-api-key", "test_key")
            .json(&json!({ "code": "console.log('hi')", "stdin": "" }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        assert_eq!(
            resp.json::<Value>().await.unwrap(),
            json!({ "stdout": "hi\n", "stderr": "", "exit_code": 0 })
        );

        // The code from the request must reach Piston under the configured runtime.
        let sent = &seen.lock().unwrap()[0];
        assert_eq!(sent["language"], "javascript");
        assert_eq!(sent["version"], "*");
        assert_eq!(sent["files"][0]["content"], "console.log('hi')");

        // Sandbox limits are only enforced if the field names match Piston's API
        // exactly -- a typo here is silently ignored and runs go unbounded.
        assert_eq!(sent["run_timeout"], 3000);
        assert_eq!(sent["run_memory_limit"], 128 * 1024 * 1024);
    }

    #[tokio::test]
    async fn execute_rejects_wrong_api_key() {
        let (piston_url, seen) = spawn_piston(StatusCode::OK, ok_run()).await;
        let url = spawn(build_router(state_for(&piston_url))).await;

        let resp = reqwest::Client::new()
            .post(format!("{url}/execute"))
            .header("x-api-key", "wrong_key")
            .json(&json!({ "code": "console.log('hi')", "stdin": "" }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 401);
        // Auth must happen before anything touches Piston.
        assert!(seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn execute_rejects_missing_api_key() {
        let (piston_url, seen) = spawn_piston(StatusCode::OK, ok_run()).await;
        let url = spawn(build_router(state_for(&piston_url))).await;

        let resp = reqwest::Client::new()
            .post(format!("{url}/execute"))
            .json(&json!({ "code": "console.log('hi')", "stdin": "" }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 401);
        assert!(seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn execute_maps_piston_error_to_bad_gateway() {
        let (piston_url, _) = spawn_piston(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "message": "boom" }),
        )
        .await;
        let url = spawn(build_router(state_for(&piston_url))).await;

        let resp = reqwest::Client::new()
            .post(format!("{url}/execute"))
            .header("x-api-key", "test_key")
            .json(&json!({ "code": "1", "stdin": "" }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 502);
    }

    #[tokio::test]
    async fn execute_maps_unreachable_piston_to_bad_gateway() {
        // Port 1 on loopback: nothing listens there.
        let url = spawn(build_router(state_for("http://127.0.0.1:1"))).await;

        let resp = reqwest::Client::new()
            .post(format!("{url}/execute"))
            .header("x-api-key", "test_key")
            .json(&json!({ "code": "1", "stdin": "" }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 502);
    }

    #[tokio::test]
    async fn execute_defaults_missing_exit_code_to_minus_one() {
        // Piston omits `code` when the process is killed by a signal.
        let (piston_url, _) = spawn_piston(
            StatusCode::OK,
            json!({ "run": { "stdout": "", "stderr": "killed", "code": null } }),
        )
        .await;
        let url = spawn(build_router(state_for(&piston_url))).await;

        let resp = reqwest::Client::new()
            .post(format!("{url}/execute"))
            .header("x-api-key", "test_key")
            .json(&json!({ "code": "while(true){}", "stdin": "" }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        assert_eq!(resp.json::<Value>().await.unwrap()["exit_code"], -1);
    }

    #[tokio::test]
    async fn execute_rejects_malformed_body() {
        let (piston_url, seen) = spawn_piston(StatusCode::OK, ok_run()).await;
        let url = spawn(build_router(state_for(&piston_url))).await;

        let resp = reqwest::Client::new()
            .post(format!("{url}/execute"))
            .header("x-api-key", "test_key")
            .json(&json!({ "stdin": "" })) // no `code`
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 422);
        assert!(seen.lock().unwrap().is_empty());
    }
}
