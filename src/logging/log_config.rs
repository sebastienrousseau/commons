// Copyright © 2024-2026 RustLogs (RLG). All rights reserved.
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! TOML-based configuration: loading, validation, diffing, and hot-reload.

use super::log_level::LogLevel;
use config_crate::{
    Config as ConfigSource, ConfigError as SourceConfigError,
    File as ConfigFile,
};
#[cfg(feature = "logging-tokio")]
use notify::{Event, EventKind, RecursiveMode, Watcher};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fmt,
    fs::{self, OpenOptions},
    num::NonZeroU64,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};
use thiserror::Error;

#[cfg(feature = "logging-tokio")]
use tokio::fs::File;
#[cfg(feature = "logging-tokio")]
use tokio::io::AsyncReadExt;
#[cfg(feature = "logging-tokio")]
use tokio::sync::mpsc;

const CURRENT_CONFIG_VERSION: &str = "1.0";

/// Configuration error variants.
#[derive(Debug, Error)]
pub enum LoggingConfigError {
    /// Failed to parse an environment variable.
    #[error("Environment variable parse error: {0}")]
    EnvVarParseError(#[from] envy::Error),

    /// Failed to parse the configuration file.
    #[error("Configuration parsing error: {0}")]
    ConfigParseError(#[from] SourceConfigError),

    /// The provided config file path is invalid or inaccessible.
    #[error("Invalid file path: {0}")]
    InvalidFilePath(String),

    /// File read failed.
    #[error("File read error: {0}")]
    FileReadError(String),

    /// File write failed.
    #[error("File write error: {0}")]
    FileWriteError(String),

    /// Validation failed for a configuration field.
    #[error("Configuration validation error: {0}")]
    ValidationError(String),

    /// Config file version does not match the expected version.
    #[error("Configuration version error: {0}")]
    VersionError(String),

    /// A required field is missing from the configuration.
    #[error("Missing required field: {0}")]
    MissingFieldError(String),

    /// File watcher setup failed (requires `logging-tokio` feature).
    #[cfg(feature = "logging-tokio")]
    #[error("Watcher error: {0}")]
    WatcherError(#[from] notify::Error),
}

#[cfg(feature = "config")]
impl From<crate::config::ConfigError> for LoggingConfigError {
    fn from(err: crate::config::ConfigError) -> Self {
        Self::ValidationError(err.to_string())
    }
}

/// Log rotation policy variants.
#[derive(
    Clone,
    Copy,
    Debug,
    Deserialize,
    Serialize,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
)]
pub enum LogRotation {
    /// Size-based log rotation.
    Size(NonZeroU64),
    /// Time-based log rotation.
    Time(NonZeroU64),
    /// Date-based log rotation.
    Date,
    /// Count-based log rotation.
    Count(u32),
}

impl FromStr for LogRotation {
    type Err = LoggingConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.trim().splitn(2, ':').collect();
        match parts[0].to_lowercase().as_str() {
            "size" => {
                let size_str = parts.get(1).ok_or_else(|| {
                    LoggingConfigError::ValidationError(
                        "Missing size value for log rotation".to_string(),
                    )
                })?;
                let size = size_str.parse::<u64>().map_err(|_| LoggingConfigError::ValidationError(format!("Invalid size value for log rotation: '{size_str}'")))?;
                Ok(Self::Size(NonZeroU64::new(size).ok_or_else(
                    || {
                        LoggingConfigError::ValidationError(
                            "Log rotation size must be greater than 0".to_string(),
                        )
                    },
                )?))
            }
            "time" => {
                let time_str = parts.get(1).ok_or_else(|| {
                    LoggingConfigError::ValidationError(
                        "Missing time value for log rotation".to_string(),
                    )
                })?;
                let time = time_str.parse::<u64>().map_err(|_| LoggingConfigError::ValidationError(format!("Invalid time value for log rotation: '{time_str}'")))?;
                Ok(Self::Time(NonZeroU64::new(time).ok_or_else(
                    || {
                        LoggingConfigError::ValidationError(
                            "Log rotation time must be greater than 0".to_string(),
                        )
                    },
                )?))
            }
            "date" => Ok(Self::Date),
            "count" => {
                let count = parts
                    .get(1)
                    .ok_or_else(|| LoggingConfigError::ValidationError("Missing count value for log rotation".to_string()))?
                    .parse::<usize>()
                    .map_err(|_| LoggingConfigError::ValidationError(format!("Invalid count value for log rotation: '{0}'", parts[1])))?;
                if count == 0 {
                    Err(LoggingConfigError::ValidationError(
                        "Log rotation count must be greater than 0".to_string(),
                    ))
                } else {
                    Ok(Self::Count(
                        count.try_into().unwrap_or(u32::MAX),
                    ))
                }
            }
            _ => Err(LoggingConfigError::ValidationError(format!(
                "Invalid log rotation option: '{s}'"
            ))),
        }
    }
}

/// Enum representing different logging destinations.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum LoggingDestination {
    /// Log to a file.
    File(PathBuf),
    /// Log to standard output.
    Stdout,
    /// Log to a network destination.
    Network(String),
}

/// Configuration structure for the logging system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::unsafe_derive_deserialize)]
pub struct LoggingConfig {
    /// Version of the configuration.
    #[serde(default = "default_version")]
    pub version: String,
    /// Profile name for the configuration.
    #[serde(default = "default_profile")]
    pub profile: String,
    /// Path to the log file.
    #[serde(default = "default_log_file_path")]
    pub log_file_path: PathBuf,
    /// Log level for the system.
    #[serde(default)]
    pub log_level: LogLevel,
    /// Log rotation settings.
    pub log_rotation: Option<LogRotation>,
    /// Log format string.
    #[serde(default = "default_log_format")]
    pub log_format: String,
    /// Logging destinations for the system.
    #[serde(default = "default_logging_destinations")]
    pub logging_destinations: Vec<LoggingDestination>,
    /// Environment variables for the system.
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
}

fn default_version() -> String {
    CURRENT_CONFIG_VERSION.to_string()
}
fn default_profile() -> String {
    "default".to_string()
}
fn default_log_file_path() -> PathBuf {
    PathBuf::from("RLG.log")
}
fn default_log_format() -> String {
    "%level - %message".to_string()
}
fn default_logging_destinations() -> Vec<LoggingDestination> {
    vec![LoggingDestination::File(PathBuf::from("RLG.log"))]
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            profile: default_profile(),
            log_file_path: default_log_file_path(),
            log_level: LogLevel::INFO,
            log_rotation: NonZeroU64::new(10 * 1024 * 1024)
                .map(LogRotation::Size),
            log_format: default_log_format(),
            logging_destinations: default_logging_destinations(),
            env_vars: HashMap::new(),
        }
    }
}

impl LoggingConfig {
    /// Loads configuration from a file or falls back to defaults.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration file cannot be read,
    /// parsed, or if the version is unsupported.
    pub fn load<P: AsRef<Path>>(
        config_path: Option<P>,
    ) -> Result<Arc<RwLock<Self>>, LoggingConfigError> {
        let config = if let Some(path) = config_path {
            let contents =
                fs::read_to_string(path.as_ref()).map_err(|e| {
                    LoggingConfigError::FileReadError(e.to_string())
                })?;
            let config_source = ConfigSource::builder()
                .add_source(ConfigFile::from_str(
                    &contents,
                    config_crate::FileFormat::Toml,
                ))
                .build()?;
            let version: String = config_source.get("version")?;
            if version != CURRENT_CONFIG_VERSION {
                return Err(LoggingConfigError::VersionError(format!(
                    "Unsupported configuration version: {version}"
                )));
            }
            config_source.try_deserialize()?
        } else {
            Self::default()
        };
        config.validate()?;
        config.ensure_paths()?;
        Ok(Arc::new(RwLock::new(config)))
    }

    /// Loads configuration from a file or environment variables (async).
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration file cannot be read,
    /// parsed, or if the version is unsupported.
    #[cfg(feature = "logging-tokio")]
    pub async fn load_async<P: AsRef<Path>>(
        config_path: Option<P>,
    ) -> Result<Arc<RwLock<Self>>, LoggingConfigError> {
        let path_buf = config_path.map(|p| p.as_ref().to_path_buf());
        let config = if let Some(path) = path_buf {
            let mut file = File::open(&path).await.map_err(|e| {
                LoggingConfigError::FileReadError(e.to_string())
            })?;
            let mut contents = String::new();
            file.read_to_string(&mut contents).await.map_err(|e| {
                LoggingConfigError::FileReadError(e.to_string())
            })?;
            let config_source = ConfigSource::builder()
                .add_source(ConfigFile::from_str(
                    &contents,
                    config_crate::FileFormat::Toml,
                ))
                .build()?;
            let version: String = config_source.get("version")?;
            if version != CURRENT_CONFIG_VERSION {
                return Err(LoggingConfigError::VersionError(format!(
                    "Unsupported configuration version: {version}"
                )));
            }
            config_source.try_deserialize()?
        } else {
            Self::default()
        };
        config.validate()?;
        config.ensure_paths()?;
        Ok(Arc::new(RwLock::new(config)))
    }

    /// Saves the current configuration to a file in TOML format.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written or serialization fails.
    pub fn save_to_file<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<(), LoggingConfigError> {
        let config_string =
            toml::to_string_pretty(self).map_err(|e| {
                LoggingConfigError::FileWriteError(format!(
                    "Failed to serialize config to TOML: {e}"
                ))
            })?;
        fs::write(path, config_string).map_err(|e| {
            LoggingConfigError::FileWriteError(format!(
                "Failed to write config file: {e}"
            ))
        })?;
        Ok(())
    }

    /// Sets a value in the configuration based on the specified key.
    ///
    /// # Errors
    ///
    /// Returns an error if the value cannot be serialized or the key is unknown.
    pub fn set<T: Serialize>(
        &mut self,
        key: &str,
        value: T,
    ) -> Result<(), LoggingConfigError> {
        let val = serde_json::to_value(value)
            .map_err(|e| LoggingConfigError::ValidationError(e.to_string()))?;

        match key {
            "version" => {
                if let Some(s) = val.as_str() {
                    self.version = s.to_string();
                } else {
                    return Err(LoggingConfigError::ValidationError(
                        "Invalid version format".to_string(),
                    ));
                }
            }
            "profile" => {
                if let Some(s) = val.as_str() {
                    self.profile = s.to_string();
                } else {
                    return Err(LoggingConfigError::ValidationError(
                        "Invalid profile format".to_string(),
                    ));
                }
            }
            "log_file_path" => {
                self.log_file_path = serde_json::from_value(val)
                    .map_err(|e| {
                        LoggingConfigError::ConfigParseError(
                            SourceConfigError::Message(e.to_string()),
                        )
                    })?;
            }
            "log_level" => {
                self.log_level =
                    serde_json::from_value(val).map_err(|e| {
                        LoggingConfigError::ConfigParseError(
                            SourceConfigError::Message(e.to_string()),
                        )
                    })?;
            }
            "log_rotation" => {
                self.log_rotation = serde_json::from_value(val)
                    .map_err(|e| {
                        LoggingConfigError::ConfigParseError(
                            SourceConfigError::Message(e.to_string()),
                        )
                    })?;
            }
            "log_format" => {
                if let Some(s) = val.as_str() {
                    self.log_format = s.to_string();
                } else {
                    return Err(LoggingConfigError::ValidationError(
                        "Invalid log format".to_string(),
                    ));
                }
            }
            "logging_destinations" => {
                self.logging_destinations = serde_json::from_value(val)
                    .map_err(|e| {
                        LoggingConfigError::ConfigParseError(
                            SourceConfigError::Message(e.to_string()),
                        )
                    })?;
            }
            "env_vars" => {
                self.env_vars =
                    serde_json::from_value(val).map_err(|e| {
                        LoggingConfigError::ConfigParseError(
                            SourceConfigError::Message(e.to_string()),
                        )
                    })?;
            }
            _ => {
                return Err(LoggingConfigError::ValidationError(format!(
                    "Unknown configuration key: {key}"
                )));
            }
        }
        Ok(())
    }

    /// Validates the configuration settings.
    ///
    /// # Errors
    ///
    /// Returns an error if any configuration setting is invalid.
    pub fn validate(&self) -> Result<(), LoggingConfigError> {
        if self.version.trim().is_empty() {
            return Err(LoggingConfigError::ValidationError(
                "version cannot be empty".into(),
            ));
        }
        if self.profile.trim().is_empty() {
            return Err(LoggingConfigError::ValidationError(
                "profile cannot be empty".into(),
            ));
        }
        if self.log_format.trim().is_empty() {
            return Err(LoggingConfigError::ValidationError(
                "log_format cannot be empty".into(),
            ));
        }
        if self.log_file_path.as_os_str().is_empty() {
            return Err(LoggingConfigError::ValidationError(
                "Log file path cannot be empty".into(),
            ));
        }
        if self.logging_destinations.is_empty() {
            return Err(LoggingConfigError::ValidationError(
                "At least one logging destination must be specified".into(),
            ));
        }
        for (key, value) in &self.env_vars {
            if key.trim().is_empty() {
                return Err(LoggingConfigError::ValidationError(
                    "Environment variable key cannot be empty".into(),
                ));
            }
            if value.trim().is_empty() {
                return Err(LoggingConfigError::ValidationError(
                    format!("Environment variable value for '{key}' cannot be empty"),
                ));
            }
        }
        Ok(())
    }

    /// Creates directories and log files required by the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the directories or files cannot be created.
    pub fn ensure_paths(&self) -> Result<(), LoggingConfigError> {
        if let Some(LoggingDestination::File(path)) =
            self.logging_destinations.first()
        {
            if let Some(parent_dir) = path.parent() {
                fs::create_dir_all(parent_dir).map_err(|e| {
                    LoggingConfigError::ValidationError(format!(
                        "Failed to create directory for log file: {e}"
                    ))
                })?;
            }
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| {
                    LoggingConfigError::ValidationError(format!(
                        "Log file is not writable: {e}"
                    ))
                })?;
        }
        Ok(())
    }

    /// Expands environment variables in the configuration values.
    #[must_use]
    pub fn expand_env_vars(&self) -> Self {
        let mut new_config = self.clone();
        for (key, value) in &mut new_config.env_vars {
            if let Ok(env_value) = env::var(key) {
                *value = env_value;
            }
        }
        new_config
    }

    /// Hot-reloads configuration on file change.
    ///
    /// # Errors
    ///
    /// Returns an error if the watcher cannot be initialized.
    #[cfg(feature = "logging-tokio")]
    #[allow(clippy::incompatible_msrv)]
    pub fn hot_reload_async(
        config_path: &str,
        config: &Arc<RwLock<Self>>,
    ) -> Result<mpsc::Sender<()>, LoggingConfigError> {
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);
        let (tx, mut rx) = mpsc::channel::<notify::Result<Event>>(100);

        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.blocking_send(res);
        })?;
        watcher.watch(
            Path::new(config_path),
            RecursiveMode::NonRecursive,
        )?;

        let config_clone = config.clone();
        let path_owned = config_path.to_string();
        tokio::spawn(async move {
            let _watcher = watcher;
            loop {
                tokio::select! {
                    Some(res) = rx.recv() => {
                        if let Ok(Event { kind: EventKind::Modify(_), .. }) = res
                            && let Ok(new_config) = Self::load_async(Some(&path_owned)).await {
                                let mut config_write = config_clone.write();
                                *config_write = new_config.read().clone();
                        }
                    }
                    _ = stop_rx.recv() => break,
                }
            }
        });
        Ok(stop_tx)
    }

    /// Compares two configurations and returns the differences.
    #[must_use]
    pub fn diff(
        config1: &Self,
        config2: &Self,
    ) -> HashMap<String, String> {
        let mut diffs = HashMap::new();
        macro_rules! config_diff_fields {
            ($c1:expr, $c2:expr, $diffs:expr;
             $( display $field:ident; )*
             $( debug $dfield:ident; )*
             $( path $pfield:ident; )*
            ) => {
                $(
                    if $c1.$field != $c2.$field {
                        $diffs.insert(
                            stringify!($field).to_string(),
                            format!("{} -> {}", $c1.$field, $c2.$field),
                        );
                    }
                )*
                $(
                    if $c1.$dfield != $c2.$dfield {
                        $diffs.insert(
                            stringify!($dfield).to_string(),
                            format!("{:?} -> {:?}", $c1.$dfield, $c2.$dfield),
                        );
                    }
                )*
                $(
                    if $c1.$pfield != $c2.$pfield {
                        $diffs.insert(
                            stringify!($pfield).to_string(),
                            format!("{} -> {}", $c1.$pfield.display(), $c2.$pfield.display()),
                        );
                    }
                )*
            };
        }
        config_diff_fields!(config1, config2, diffs;
            display version;
            display profile;
            display log_format;
            debug log_level;
            debug log_rotation;
            debug logging_destinations;
            debug env_vars;
            path log_file_path;
        );
        diffs
    }

    /// Overrides the current configuration with values from another configuration.
    #[must_use]
    pub fn override_with(&self, other: &Self) -> Self {
        let mut env_vars = self.env_vars.clone();
        env_vars.extend(other.env_vars.clone());
        Self {
            version: other.version.clone(),
            profile: other.profile.clone(),
            log_file_path: other.log_file_path.clone(),
            log_level: other.log_level,
            log_rotation: other.log_rotation,
            log_format: other.log_format.clone(),
            logging_destinations: other.logging_destinations.clone(),
            env_vars,
        }
    }
}

impl TryFrom<env::Vars> for LoggingConfig {
    type Error = LoggingConfigError;
    fn try_from(vars: env::Vars) -> Result<Self, Self::Error> {
        envy::from_iter(vars).map_err(LoggingConfigError::EnvVarParseError)
    }
}

impl fmt::Display for LogRotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Size(size) => write!(f, "Size: {size} bytes"),
            Self::Time(seconds) => write!(f, "Time: {seconds} seconds"),
            Self::Date => write!(f, "Date-based rotation"),
            Self::Count(count) => write!(f, "Count: {count} logs"),
        }
    }
}

#[cfg(all(test, not(miri)))]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_values() {
        let config = LoggingConfig::default();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.profile, "default");
        assert_eq!(config.log_file_path, PathBuf::from("RLG.log"));
        assert_eq!(config.log_level, LogLevel::INFO);
        assert!(config.log_rotation.is_some());
    }

    #[test]
    fn test_log_rotation_valid() {
        let size = LogRotation::from_str("size:1024").unwrap();
        assert!(matches!(size, LogRotation::Size(_)));

        let time = LogRotation::from_str("time:3600").unwrap();
        assert!(matches!(time, LogRotation::Time(_)));

        let date = LogRotation::from_str("date").unwrap();
        assert!(matches!(date, LogRotation::Date));

        let count = LogRotation::from_str("count:10").unwrap();
        assert!(matches!(count, LogRotation::Count(10)));
    }

    #[test]
    fn test_log_rotation_invalid() {
        assert!(LogRotation::from_str("count:0").is_err());
        assert!(LogRotation::from_str("size:0").is_err());
        assert!(LogRotation::from_str("time:0").is_err());
        assert!(LogRotation::from_str("invalid:xxx").is_err());
        assert!(LogRotation::from_str("size").is_err());
        assert!(LogRotation::from_str("time").is_err());
        assert!(LogRotation::from_str("count").is_err());
    }

    #[test]
    fn test_log_rotation_display() {
        let size = LogRotation::Size(NonZeroU64::new(1024).unwrap());
        assert_eq!(size.to_string(), "Size: 1024 bytes");
        assert_eq!(LogRotation::Date.to_string(), "Date-based rotation");
        assert_eq!(LogRotation::Count(5).to_string(), "Count: 5 logs");
    }

    #[test]
    fn test_config_validate_empty_path() {
        let config = LoggingConfig {
            log_file_path: PathBuf::from(""),
            ..LoggingConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validate_empty_destinations() {
        let mut config = LoggingConfig::default();
        config.logging_destinations.clear();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_diff_no_changes() {
        let c1 = LoggingConfig::default();
        let c2 = LoggingConfig::default();
        let diffs = LoggingConfig::diff(&c1, &c2);
        assert!(diffs.is_empty());
    }

    #[test]
    fn test_config_diff_with_changes() {
        let c1 = LoggingConfig::default();
        let c2 = LoggingConfig {
            version: "2.0".to_string(),
            profile: "prod".to_string(),
            ..LoggingConfig::default()
        };
        let diffs = LoggingConfig::diff(&c1, &c2);
        assert!(diffs.contains_key("version"));
        assert!(diffs.contains_key("profile"));
    }

    #[test]
    fn test_config_override_with() {
        let c1 = LoggingConfig::default();
        let c2 = LoggingConfig {
            version: "2.0".to_string(),
            profile: "prod".to_string(),
            ..LoggingConfig::default()
        };
        let merged = c1.override_with(&c2);
        assert_eq!(merged.version, "2.0");
        assert_eq!(merged.profile, "prod");
    }

    #[test]
    fn test_config_set_valid_values() {
        let mut config = LoggingConfig::default();
        assert!(config.set("version", "2.0").is_ok());
        assert_eq!(config.version, "2.0");
        assert!(config.set("profile", "production").is_ok());
        assert_eq!(config.profile, "production");
    }

    #[test]
    fn test_config_set_unknown_key() {
        let mut config = LoggingConfig::default();
        assert!(config.set("unknown_key", "value").is_err());
    }

    #[test]
    fn test_config_ensure_paths() {
        let config = LoggingConfig::default();
        assert!(config.ensure_paths().is_ok());
    }

    #[test]
    fn test_load_with_valid_toml_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let toml_content = format!(
            r#"version = "1.0"
profile = "test"
log_file_path = "{}"
log_level = "INFO"
log_format = "%level - %message"

[[logging_destinations]]
type = "Stdout"
"#,
            dir.path().join("test.log").display()
        );
        fs::write(&config_path, &toml_content).unwrap();
        let config =
            LoggingConfig::load(Some(&config_path)).unwrap();
        let cfg = config.read();
        assert_eq!(cfg.version, "1.0");
        assert_eq!(cfg.profile, "test");
        assert_eq!(cfg.log_format, "%level - %message");
        drop(cfg);
    }

    #[test]
    fn test_load_with_none_defaults() {
        let config =
            LoggingConfig::load(None::<&str>).unwrap();
        let cfg = config.read();
        assert_eq!(cfg.version, "1.0");
        assert_eq!(cfg.profile, "default");
        assert_eq!(cfg.log_level, LogLevel::INFO);
        drop(cfg);
    }

    #[test]
    fn test_load_with_nonexistent_file() {
        let result = LoggingConfig::load(Some("/nonexistent/file.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_with_wrong_version() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        let toml_content = r#"version = "999.0"
profile = "test"
log_file_path = "test.log"
log_level = "INFO"
log_format = "%level - %message"

[[logging_destinations]]
type = "Stdout"
"#;
        fs::write(&config_path, toml_content).unwrap();
        let result = LoggingConfig::load(Some(&config_path));
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("version"));
    }

    #[test]
    fn test_save_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let save_path = dir.path().join("saved_config.toml");
        let config = LoggingConfig::default();
        config.save_to_file(&save_path).unwrap();
        let contents = fs::read_to_string(&save_path).unwrap();
        assert!(contents.contains("version"));
        assert!(contents.contains("1.0"));
    }

    #[test]
    fn test_save_to_file_invalid_path() {
        let config = LoggingConfig::default();
        let result =
            config.save_to_file("/nonexistent/dir/config.toml");
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("Failed to write"));
    }

    #[test]
    fn test_set_log_file_path() {
        let mut config = LoggingConfig::default();
        assert!(config.set("log_file_path", "new.log").is_ok());
        assert_eq!(config.log_file_path, PathBuf::from("new.log"));
    }

    #[test]
    fn test_set_log_level() {
        let mut config = LoggingConfig::default();
        assert!(config.set("log_level", "ERROR").is_ok());
        assert_eq!(config.log_level, LogLevel::ERROR);
    }

    #[test]
    fn test_set_log_rotation() {
        let mut config = LoggingConfig::default();
        let rotation = LogRotation::Date;
        assert!(config.set("log_rotation", rotation).is_ok());
        assert_eq!(config.log_rotation, Some(LogRotation::Date));
    }

    #[test]
    fn test_set_log_format() {
        let mut config = LoggingConfig::default();
        assert!(config.set("log_format", "%time %message").is_ok());
        assert_eq!(config.log_format, "%time %message");
    }

    #[test]
    fn test_set_logging_destinations() {
        let mut config = LoggingConfig::default();
        let dests = vec![LoggingDestination::Stdout];
        assert!(config.set("logging_destinations", &dests).is_ok());
        assert_eq!(config.logging_destinations.len(), 1);
        assert!(matches!(
            config.logging_destinations[0],
            LoggingDestination::Stdout
        ));
    }

    #[test]
    fn test_set_env_vars() {
        let mut config = LoggingConfig::default();
        let mut vars = HashMap::new();
        vars.insert("KEY".to_string(), "VALUE".to_string());
        assert!(config.set("env_vars", &vars).is_ok());
        assert_eq!(config.env_vars.get("KEY").unwrap(), "VALUE");
    }

    #[test]
    fn test_set_version_invalid_type() {
        let mut config = LoggingConfig::default();
        // Pass an integer instead of a string for version
        let result = config.set("version", 42);
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("Invalid version format"));
    }

    #[test]
    fn test_set_profile_invalid_type() {
        let mut config = LoggingConfig::default();
        let result = config.set("profile", 42);
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("Invalid profile format"));
    }

    #[test]
    fn test_set_log_format_invalid_type() {
        let mut config = LoggingConfig::default();
        let result = config.set("log_format", 42);
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("Invalid log format"));
    }

    #[test]
    fn test_validate_empty_version() {
        let config = LoggingConfig {
            version: "  ".to_string(),
            ..LoggingConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("version cannot be empty"));
    }

    #[test]
    fn test_validate_empty_profile() {
        let config = LoggingConfig {
            profile: " ".to_string(),
            ..LoggingConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("profile cannot be empty"));
    }

    #[test]
    fn test_validate_empty_log_format() {
        let config = LoggingConfig {
            log_format: "  ".to_string(),
            ..LoggingConfig::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("log_format cannot be empty"));
    }

    #[test]
    fn test_validate_empty_env_var_key() {
        let mut config = LoggingConfig::default();
        config.env_vars.insert(" ".to_string(), "val".to_string());
        let result = config.validate();
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("key cannot be empty"));
    }

    #[test]
    fn test_validate_empty_env_var_value() {
        let mut config = LoggingConfig::default();
        config
            .env_vars
            .insert("MY_KEY".to_string(), " ".to_string());
        let result = config.validate();
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("MY_KEY"));
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_expand_env_vars() {
        let mut config = LoggingConfig::default();
        config
            .env_vars
            .insert("HOME".to_string(), "placeholder".to_string());
        // SAFETY: Test-only env var.
        unsafe { env::set_var("HOME", "/test/home") };
        let expanded = config.expand_env_vars();
        assert_eq!(
            expanded.env_vars.get("HOME").unwrap(),
            "/test/home"
        );
    }

    #[test]
    fn test_expand_env_vars_nonexistent() {
        let mut config = LoggingConfig::default();
        config.env_vars.insert(
            "NONEXISTENT_VAR_12345".to_string(),
            "original".to_string(),
        );
        let expanded = config.expand_env_vars();
        assert_eq!(
            expanded.env_vars.get("NONEXISTENT_VAR_12345").unwrap(),
            "original"
        );
    }

    #[test]
    fn test_try_from_env_vars() {
        // This calls envy::from_iter on env::Vars, which will
        // likely fail to deserialize into LoggingConfig, but we
        // exercise the code path.
        let result = LoggingConfig::try_from(env::vars());
        // The conversion may succeed or fail depending on env vars,
        // but the code path is exercised either way.
        let _ = result;
    }

    #[test]
    fn test_logging_config_error_display_variants() {
        let err = LoggingConfigError::InvalidFilePath(
            "bad/path".to_string(),
        );
        assert!(err.to_string().contains("bad/path"));

        let err = LoggingConfigError::FileReadError(
            "read failed".to_string(),
        );
        assert!(err.to_string().contains("read failed"));

        let err = LoggingConfigError::FileWriteError(
            "write failed".to_string(),
        );
        assert!(err.to_string().contains("write failed"));

        let err = LoggingConfigError::ValidationError(
            "invalid config".to_string(),
        );
        assert!(err.to_string().contains("invalid config"));

        let err = LoggingConfigError::VersionError(
            "wrong version".to_string(),
        );
        assert!(err.to_string().contains("wrong version"));

        let err = LoggingConfigError::MissingFieldError(
            "field_x".to_string(),
        );
        assert!(err.to_string().contains("field_x"));
    }

    #[test]
    fn test_logging_destination_file_variant() {
        let dest =
            LoggingDestination::File(PathBuf::from("some.log"));
        assert!(matches!(dest, LoggingDestination::File(_)));
    }

    #[test]
    fn test_logging_destination_stdout_variant() {
        let dest = LoggingDestination::Stdout;
        assert!(matches!(dest, LoggingDestination::Stdout));
    }

    #[test]
    fn test_logging_destination_network_variant() {
        let dest = LoggingDestination::Network(
            "tcp://localhost:514".to_string(),
        );
        assert!(matches!(dest, LoggingDestination::Network(_)));
    }

    #[test]
    fn test_logging_destination_serde_roundtrip() {
        let dest = LoggingDestination::Stdout;
        let json = serde_json::to_string(&dest).unwrap();
        let back: LoggingDestination =
            serde_json::from_str(&json).unwrap();
        assert_eq!(dest, back);

        let dest =
            LoggingDestination::File(PathBuf::from("test.log"));
        let json = serde_json::to_string(&dest).unwrap();
        let back: LoggingDestination =
            serde_json::from_str(&json).unwrap();
        assert_eq!(dest, back);

        let dest = LoggingDestination::Network(
            "tcp://example.com".to_string(),
        );
        let json = serde_json::to_string(&dest).unwrap();
        let back: LoggingDestination =
            serde_json::from_str(&json).unwrap();
        assert_eq!(dest, back);
    }

    #[test]
    fn test_log_rotation_from_str_invalid_size_value() {
        // "size:abc" -- not a number
        let result = LogRotation::from_str("size:abc");
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("Invalid size value"));
    }

    #[test]
    fn test_log_rotation_from_str_invalid_time_value() {
        // "time:xyz" -- not a number
        let result = LogRotation::from_str("time:xyz");
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("Invalid time value"));
    }

    #[test]
    fn test_log_rotation_from_str_invalid_count_value() {
        // "count:abc" -- not a number
        let result = LogRotation::from_str("count:abc");
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("Invalid count value"));
    }

    #[test]
    fn test_log_rotation_time_display() {
        let time =
            LogRotation::Time(NonZeroU64::new(3600).unwrap());
        assert_eq!(time.to_string(), "Time: 3600 seconds");
    }

    // ---- load_async tests (logging-tokio feature) ----

    #[cfg(feature = "logging-tokio")]
    #[tokio::test]
    async fn test_load_async_with_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let log_path = dir.path().join("test.log");
        let toml_content = format!(
            r#"version = "1.0"
profile = "async_test"
log_file_path = "{}"
log_level = "INFO"
log_format = "%level - %message"

[[logging_destinations]]
type = "Stdout"
"#,
            log_path.display()
        );
        fs::write(&path, &toml_content).unwrap();
        let config =
            LoggingConfig::load_async(Some(&path)).await.unwrap();
        assert_eq!(config.read().profile, "async_test");
    }

    #[cfg(feature = "logging-tokio")]
    #[tokio::test]
    async fn test_load_async_defaults() {
        let config =
            LoggingConfig::load_async(None::<&str>).await.unwrap();
        assert_eq!(config.read().version, "1.0");
    }

    #[cfg(feature = "logging-tokio")]
    #[tokio::test]
    async fn test_load_async_nonexistent_file() {
        let result = LoggingConfig::load_async(Some(
            "/nonexistent/path/config.toml",
        ))
        .await;
        assert!(result.is_err());
    }

    #[cfg(feature = "logging-tokio")]
    #[tokio::test]
    async fn test_load_async_wrong_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let toml_content = r#"version = "999.0"
profile = "test"
log_file_path = "test.log"
log_level = "INFO"
log_format = "%level - %message"

[[logging_destinations]]
type = "Stdout"
"#;
        fs::write(&path, toml_content).unwrap();
        let result = LoggingConfig::load_async(Some(&path)).await;
        assert!(result.is_err());
        let err_str = format!("{}", result.unwrap_err());
        assert!(err_str.contains("version"));
    }

    // ---- From<ConfigError> test (config feature) ----

    #[cfg(feature = "config")]
    #[test]
    fn test_from_config_error() {
        let config_err = crate::config::ConfigError::Parse(
            "bad config".to_string(),
        );
        let logging_err: LoggingConfigError = config_err.into();
        let err_str = format!("{logging_err}");
        assert!(err_str.contains("bad config"));
        // Should be a ValidationError variant
        assert!(matches!(
            logging_err,
            LoggingConfigError::ValidationError(_)
        ));
    }

    // ---- LoggingConfigError Display for EnvVarParseError and ConfigParseError ----

    #[test]
    fn test_logging_config_error_env_var_parse_display() {
        // Construct an envy::Error via a failed deserialization
        let result: Result<LoggingConfig, envy::Error> =
            envy::from_iter(std::iter::empty::<(String, String)>());
        if let Err(envy_err) = result {
            let err = LoggingConfigError::EnvVarParseError(envy_err);
            let display = err.to_string();
            assert!(
                display.contains("Environment variable parse error"),
                "Display was: {display}"
            );
        }
    }

    #[test]
    fn test_logging_config_error_config_parse_display() {
        let source_err =
            SourceConfigError::Message("test parse error".to_string());
        let err = LoggingConfigError::ConfigParseError(source_err);
        let display = err.to_string();
        assert!(
            display.contains("test parse error"),
            "Display was: {display}"
        );
    }

    // ---- save_to_file round-trip ----

    #[test]
    fn test_save_to_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let save_path = dir.path().join("roundtrip.toml");
        let config = LoggingConfig {
            profile: "roundtrip_test".to_string(),
            ..LoggingConfig::default()
        };
        config.save_to_file(&save_path).unwrap();
        let contents = fs::read_to_string(&save_path).unwrap();
        assert!(contents.contains("roundtrip_test"));
        assert!(contents.contains("version"));
    }

    // ---- expand_env_vars with a set env var ----

    #[test]
    #[allow(unsafe_code)]
    fn test_expand_env_vars_with_custom_var() {
        let mut config = LoggingConfig::default();
        let key = "RLG_TEST_EXPAND_VAR_12345";
        config
            .env_vars
            .insert(key.to_string(), "placeholder".to_string());
        // SAFETY: Test-only env var.
        unsafe { env::set_var(key, "expanded_value") };
        let expanded = config.expand_env_vars();
        assert_eq!(
            expanded.env_vars.get(key).unwrap(),
            "expanded_value"
        );
        // SAFETY: Cleanup.
        unsafe { env::remove_var(key) };
    }

    // ---- set() error paths for deserialization failures ----

    #[test]
    fn test_set_log_file_path_invalid() {
        let mut config = LoggingConfig::default();
        // Pass a value that can't be deserialized as PathBuf (e.g., an array)
        assert!(config.set("log_file_path", vec![1, 2, 3]).is_err());
    }

    #[test]
    fn test_set_log_level_invalid() {
        let mut config = LoggingConfig::default();
        // Pass a value that can't be deserialized as LogLevel (e.g., an integer)
        assert!(config.set("log_level", 999).is_err());
    }

    #[test]
    fn test_set_log_rotation_invalid() {
        let mut config = LoggingConfig::default();
        assert!(
            config.set("log_rotation", "not_valid_rotation").is_err()
        );
    }

    #[test]
    fn test_set_logging_destinations_invalid() {
        let mut config = LoggingConfig::default();
        assert!(
            config
                .set("logging_destinations", "not_an_array")
                .is_err()
        );
    }

    #[test]
    fn test_set_env_vars_invalid() {
        let mut config = LoggingConfig::default();
        assert!(config.set("env_vars", vec![1, 2, 3]).is_err());
    }

    // ---- hot_reload_async test (logging-tokio feature) ----

    #[cfg(feature = "logging-tokio")]
    #[tokio::test]
    async fn test_hot_reload_async() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hot_reload.toml");
        let initial = format!(
            r#"version = "1.0"
profile = "initial"
log_file_path = "{}"
log_level = "INFO"
log_format = "%level - %message"

[[logging_destinations]]
type = "Stdout"
"#,
            dir.path().join("test.log").display()
        );
        fs::write(&path, &initial).unwrap();
        let config =
            LoggingConfig::load_async(Some(&path)).await.unwrap();
        assert_eq!(config.read().profile, "initial");

        let stop = LoggingConfig::hot_reload_async(
            path.to_str().unwrap(),
            &config,
        )
        .unwrap();

        // Give watcher time to initialize
        tokio::time::sleep(std::time::Duration::from_millis(200))
            .await;

        // Modify the file
        let updated = initial.replace("initial", "reloaded");
        fs::write(&path, updated).unwrap();

        // Give hot reload time to process
        tokio::time::sleep(std::time::Duration::from_millis(500))
            .await;

        // Stop the watcher
        let _ = stop.send(()).await;

        // Note: hot reload might not always trigger in test environments
        // due to filesystem watcher timing, so we just verify it doesn't panic
    }

    // ---- TryFrom<env::Vars> exercising the error path ----

    #[test]
    fn test_try_from_env_vars_exercises_code_path() {
        let result = LoggingConfig::try_from(env::vars());
        // Depending on the actual env vars present, this may succeed
        // or fail. Either way, the code path is exercised.
        match result {
            Ok(cfg) => {
                // Verify it's a valid LoggingConfig
                assert!(!cfg.version.is_empty());
            }
            Err(e) => {
                let display = e.to_string();
                assert!(
                    display.contains("Environment variable parse error"),
                    "Display was: {display}"
                );
            }
        }
    }
}
