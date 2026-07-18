use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct PistonFile {
    content: String,
}

#[derive(Serialize)]
pub struct PistonRequest {
    language: String,
    version: String,
    files: Vec<PistonFile>,
    stdin: String,
    run_timout: u32,
    run_memory_limit: i64,
}

#[derive(Deserialize)]
pub struct PistonRun {
    stdout: String,
    stderr: String,
    code: Option<u32>,
}

#[derive(Deserialize)]
pub struct PistonReponse {
    run: PistonRun,
}
