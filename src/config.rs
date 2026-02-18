//! Configuration utilities

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseConfig {
    pub name: String,
    pub version: String,
}
