# Commons

Shared Rust utilities and common patterns for the Sebastien Rousseau ecosystem.

## Overview

Commons provides reusable components, traits, and utilities used across multiple Rust projects in the ecosystem. It serves as the foundation library for consistent error handling, configuration management, logging, and more.

## Features

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `config` | TOML configuration loading | `serde`, `toml` |
| `error` | Common error types with `thiserror` | `thiserror` |
| `logging` | Simple structured logging | - |
| `time` | Duration parsing and formatting | - |
| `collections` | LRU cache and utilities | - |

By default, all features are enabled. Use `default-features = false` to select specific features.

## Installation

```toml
[dependencies]
commons = { git = "https://github.com/sebastienrousseau/commons" }

# Or with specific features only
commons = { git = "https://github.com/sebastienrousseau/commons", default-features = false, features = ["error", "time"] }
```

## Usage

### Error Handling

```rust
use commons::error::{CommonError, CommonResult};

fn process_data(input: &str) -> CommonResult<String> {
    if input.is_empty() {
        return Err(CommonError::invalid_input("Input cannot be empty"));
    }
    Ok(input.to_uppercase())
}
```

### Configuration

```rust
use commons::config::Config;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct AppConfig {
    name: String,
    port: u16,
}

let config: AppConfig = Config::from_file("config.toml")?.parse()?;
```

### Time Utilities

```rust
use commons::time::{format_duration, parse_duration};
use std::time::Duration;

let duration = parse_duration("5m").unwrap();
assert_eq!(duration, Duration::from_secs(300));

let formatted = format_duration(Duration::from_secs(3665));
assert_eq!(formatted, "1h 1m");
```

### LRU Cache

```rust
use commons::collections::LruCache;

let mut cache = LruCache::new(100);
cache.insert("key", "value");

if let Some(value) = cache.get(&"key") {
    println!("Found: {}", value);
}
```

### Logging

```rust
use commons::logging::{Logger, LogLevel};

let mut logger = Logger::new("my_module");
logger.set_level(LogLevel::Debug);

logger.info("Application started");
logger.debug("Debug information");
```

## Prelude

For convenience, import common types with the prelude:

```rust
use commons::prelude::*;
```

## Minimum Supported Rust Version

This crate requires Rust 1.70 or later.

## License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache-2.0](LICENSE-APACHE).
