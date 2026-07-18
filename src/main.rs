use crate::app::AppState;
use axum::{
    Router,
    routing::{get, post},
};

mod app;

#[tokio::main]
async fn main() {
    let api_key = std::env::var("GATEWAY_API_KEY").unwrap_or_else(|_| "dev_key".into());
    let state = AppState::new(api_key);

    let app = Router::new()
        .route("/health", get(health))
        .route("/execute", post(execute))
        .with_state(state);
}

async fn health() -> &'static str {
    "ok"
}

async fn execute() {}
