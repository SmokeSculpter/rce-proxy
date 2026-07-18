use crate::app::AppState;
use axum::{
    Router,
    routing::{get, post},
};

mod app;

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

async fn health() -> &'static str {
    "ok"
}

async fn execute() {}
