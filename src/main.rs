use crate::app::{AppState, ExecuteReponse, ExecuteRequest};
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

    // Crate app instance an add routes
    let app = build_router(state);

    // Bind listener to port 8080
    let address = "0.0.0.0:8080";
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    println!("Listening on 8080");

    // Start server with axum gateway
    axum::serve(listener, app).await.unwrap();
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
        .route("execute", post(execute))
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
) -> Result<Json<ExecuteReponse>, StatusCode> {
    let provided = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided != state.api_key.as_str() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Just a stubout for now will forward to piston instance later
    Ok(Json(ExecuteReponse {
        stdout: format!("received {} bytes on code", req.code.len()),
        stderr: String::new(),
        exit_code: 0,
    }))
}
