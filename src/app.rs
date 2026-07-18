use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct AppState {
    pub api_key: Arc<String>,
    pub http: reqwest::Client,
    pub piston_url: Arc<String>,
    pub language: Arc<String>,
    pub version: Arc<String>,
}

#[derive(Deserialize)]
pub struct ExecuteRequest {
    pub code: String,
    #[allow(dead_code)]
    pub stdin: String, // Will use for another project might as well add now
}

#[derive(Serialize)]
pub struct ExecuteResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl AppState {
    pub fn new(
        api_key: String,
        http: reqwest::Client,
        url: String,
        language: String,
        version: String,
    ) -> Self {
        Self {
            api_key: Arc::new(api_key),
            http: http,
            piston_url: Arc::new(url),
            language: Arc::new(language),
            version: Arc::new(version),
        }
    }
}
