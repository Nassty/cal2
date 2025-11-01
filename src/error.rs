use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CalError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("serialization error: {0}")]
    Bincode(#[from] bincode::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("invalid date: {0}")]
    InvalidDate(String),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("cache error: {0}")]
    Cache(String),
}

pub type Result<T> = std::result::Result<T, CalError>;
