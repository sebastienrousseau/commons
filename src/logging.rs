//! Structured logging and telemetry utilities.

use std::fmt;

/// Log levels for structured logging
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Trace level - very verbose debugging
    Trace = 0,
    /// Debug level - debugging information
    Debug = 1,
    /// Info level - general information
    Info = 2,
    /// Warn level - warning messages
    Warn = 3,
    /// Error level - error messages
    Error = 4,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// Simple structured logger
#[derive(Debug)]
pub struct Logger {
    level: LogLevel,
    module: String,
}

impl Logger {
    /// Create a new logger for a module
    pub fn new(module: &str) -> Self {
        Self {
            level: LogLevel::Info,
            module: module.to_string(),
        }
    }

    /// Set the minimum log level
    pub fn set_level(&mut self, level: LogLevel) {
        self.level = level;
    }

    /// Log a message at the given level
    pub fn log(&self, level: LogLevel, message: &str) {
        if level >= self.level {
            let timestamp = crate::time::unix_timestamp();
            println!(
                "[{}] {} [{}] {}",
                timestamp, level, self.module, message
            );
        }
    }

    /// Log a trace message
    pub fn trace(&self, message: &str) {
        self.log(LogLevel::Trace, message);
    }

    /// Log a debug message
    pub fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }

    /// Log an info message
    pub fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }

    /// Log a warning message
    pub fn warn(&self, message: &str) {
        self.log(LogLevel::Warn, message);
    }

    /// Log an error message
    pub fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }
}

/// Create a logger for the current module
#[macro_export]
macro_rules! logger {
    () => {
        $crate::logging::Logger::new(module_path!())
    };
}