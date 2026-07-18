use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct AppState {
    api_key: Arc<String>,
}

#[derive(Deserialize)]
pub struct ExecuteRequest {
    code: String,
    #[allow(dead_code)]
    stdin: String, // Will use for another project might as well add now
}

#[derive(Serialize)]
pub struct ExecuteReponse {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

impl AppState {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key: Arc::new(api_key),
        }
    }
}
