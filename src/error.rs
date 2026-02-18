//! Common error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CommonError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
