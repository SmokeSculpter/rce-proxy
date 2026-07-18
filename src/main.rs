use crate::app::{AppState, ExecuteReponse, ExecuteRequest};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};

mod app;
mod piston;

#[tokio::main]
async fn main() {
    // Get key and init state
    let api_key = std::env::var("GATEWAY_API_KEY").unwrap_or_else(|_| "dev_key".into());
    let state = AppState::new(api_key);

    // Crate app instance an add routes
    let app = Router::new()
        .route("/health", get(health))
        .route("/execute", post(execute))
        .with_state(state);

    // Bind listener to port 8080
    let address = "0.0.0.0:8080";
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    println!("Listening on 8080");

    // Start server with axum gateway
    axum::serve(listener, app).await.unwrap();
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
