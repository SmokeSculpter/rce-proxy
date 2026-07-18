use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct PistonFile {
    pub content: String,
}

#[derive(Serialize)]
pub struct PistonRequest {
    pub language: String,
    pub version: String,
    pub files: Vec<PistonFile>,
    pub stdin: String,
    pub run_timeout: u32,
    pub run_memory_limit: i64,
}

#[derive(Deserialize)]
pub struct PistonRun {
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
}

#[derive(Deserialize)]
pub struct PistonReponse {
    pub run: PistonRun,
}
