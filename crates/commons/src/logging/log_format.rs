// Copyright © 2024-2026 RustLogs (RLG). All rights reserved.
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! 14 structured output formats (JSON, MCP, OTLP, ECS, CEF, ...).

use super::log_error::{LoggingError, LoggingResult};
use super::utils::sanitize_log_message;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;

/// Compiled regular expressions for log format validation.
static CLF_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r#"^(?P<host>\S+) (?P<ident>\S+) (?P<user>\S+) \[(?P<time>[^\]]+)\] "(?P<method>\S+) (?P<path>\S+) (?P<protocol>\S+)" (?P<status>\d{3}) (?P<size>\d+|-)$"#
).expect("Failed to compile CLF regex")
});

static CEF_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^CEF:\d+\|[^|]+\|[^|]+\|[^|]+\|[^|]+\|[^|]+\|[^|]+\|.*$")
        .expect("Failed to compile CEF regex")
});

static W3C_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^#Fields:.*
.+$",
    )
    .expect("Failed to compile W3C regex")
});

/// `LogFormat` is an enum representing the different structured log formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LogFormat {
    /// Common Log Format (CLF)
    CLF,
    /// JavaScript Object Notation (JSON)
    JSON,
    /// Common Event Format (CEF)
    CEF,
    /// Extended Log Format (ELF)
    ELF,
    /// W3C Extended Log Format (W3C)
    W3C,
    /// Graylog Extended Log Format (GELF)
    GELF,
    /// Apache Access Log Format
    ApacheAccessLog,
    /// Logstash Format
    Logstash,
    /// Log4j XML Format
    Log4jXML,
    /// Network Data JSON (NDJSON)
    NDJSON,
    /// Model Context Protocol (MCP) - AI Native
    MCP,
    /// OpenTelemetry Logging (OTLP) - AI Native
    OTLP,
    /// Logfmt (key=value)
    Logfmt,
    /// Elastic Common Schema (ECS)
    ECS,
}

macro_rules! define_log_format_strings {
    ( $( $variant:ident => $display:expr, [ $( $key:expr ),+ ] );+ $(;)? ) => {
        impl FromStr for LogFormat {
            type Err = LoggingError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s.to_lowercase().as_str() {
                    $(
                        $( $key )|+ => Ok(Self::$variant),
                    )+
                    _ => Err(LoggingError::FormatParseError(format!(
                        "Unknown log format: {s}"
                    ))),
                }
            }
        }

        impl fmt::Display for LogFormat {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let s = match self {
                    $( Self::$variant => $display, )+
                };
                write!(f, "{s}")
            }
        }
    };
}

define_log_format_strings! {
    CLF => "CLF", ["clf"];
    JSON => "JSON", ["json"];
    CEF => "CEF", ["cef"];
    ELF => "ELF", ["elf"];
    W3C => "W3C", ["w3c"];
    GELF => "GELF", ["gelf"];
    ApacheAccessLog => "Apache Access Log", ["apache", "apacheaccesslog"];
    Logstash => "Logstash", ["logstash"];
    Log4jXML => "Log4j XML", ["log4jxml"];
    NDJSON => "NDJSON", ["ndjson"];
    MCP => "MCP", ["mcp"];
    OTLP => "OTLP", ["otlp"];
    Logfmt => "logfmt", ["logfmt"];
    ECS => "ECS", ["ecs"];
}

impl LogFormat {
    /// Validates a log entry against the current format.
    #[must_use]
    pub fn validate(&self, entry: &str) -> bool {
        if entry.is_empty() {
            return false;
        }
        match self {
            Self::CLF => CLF_REGEX.is_match(entry),
            Self::CEF => CEF_REGEX.is_match(entry),
            Self::W3C => W3C_REGEX.is_match(entry),
            Self::JSON
            | Self::GELF
            | Self::Logstash
            | Self::NDJSON
            | Self::MCP
            | Self::OTLP
            | Self::ECS => serde_json::from_str::<serde_json::Value>(entry).is_ok(),
            Self::Logfmt => entry.contains('=') && !entry.starts_with('='),
            Self::Log4jXML => entry.contains("<log4j:event") && entry.contains('>'),
            Self::ELF | Self::ApacheAccessLog => true,
        }
    }

    /// Formats a log entry according to the log format.
    ///
    /// # Errors
    ///
    /// Returns an error if the log entry is not valid JSON for JSON-based formats.
    ///
    /// # Panics
    ///
    /// This function does not panic under normal usage. The internal `expect` guards
    /// a `serde_json::to_string_pretty` call on a successfully parsed `Value`, which
    /// can only fail on out-of-memory conditions.
    pub fn format_log(&self, entry: &str) -> LoggingResult<String> {
        let sanitized_entry = sanitize_log_message(entry);
        match self {
            Self::CLF
            | Self::ApacheAccessLog
            | Self::CEF
            | Self::ELF
            | Self::W3C
            | Self::Log4jXML
            | Self::Logfmt => Ok(sanitized_entry),
            Self::JSON
            | Self::Logstash
            | Self::NDJSON
            | Self::GELF
            | Self::MCP
            | Self::OTLP
            | Self::ECS => {
                let val = serde_json::from_str::<serde_json::Value>(&sanitized_entry)
                    .map_err(|e| LoggingError::FormattingError(format!("Invalid JSON: {e}")))?;
                Ok(serde_json::to_string_pretty(&val)
                    .expect("serde_json::to_string_pretty cannot fail on a valid Value"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ================================================================
    // FromStr – every variant (case-insensitive)
    // ================================================================

    #[test]
    fn test_from_str_clf() {
        assert_eq!(LogFormat::from_str("clf").unwrap(), LogFormat::CLF);
        assert_eq!(LogFormat::from_str("CLF").unwrap(), LogFormat::CLF);
    }

    #[test]
    fn test_from_str_json() {
        assert_eq!(LogFormat::from_str("json").unwrap(), LogFormat::JSON);
        assert_eq!(LogFormat::from_str("JSON").unwrap(), LogFormat::JSON);
    }

    #[test]
    fn test_from_str_cef() {
        assert_eq!(LogFormat::from_str("cef").unwrap(), LogFormat::CEF);
        assert_eq!(LogFormat::from_str("CEF").unwrap(), LogFormat::CEF);
    }

    #[test]
    fn test_from_str_elf() {
        assert_eq!(LogFormat::from_str("elf").unwrap(), LogFormat::ELF);
    }

    #[test]
    fn test_from_str_w3c() {
        assert_eq!(LogFormat::from_str("w3c").unwrap(), LogFormat::W3C);
    }

    #[test]
    fn test_from_str_gelf() {
        assert_eq!(LogFormat::from_str("gelf").unwrap(), LogFormat::GELF);
    }

    #[test]
    fn test_from_str_apache() {
        assert_eq!(
            LogFormat::from_str("apache").unwrap(),
            LogFormat::ApacheAccessLog
        );
    }

    #[test]
    fn test_from_str_apacheaccesslog() {
        assert_eq!(
            LogFormat::from_str("apacheaccesslog").unwrap(),
            LogFormat::ApacheAccessLog
        );
    }

    #[test]
    fn test_from_str_logstash() {
        assert_eq!(
            LogFormat::from_str("logstash").unwrap(),
            LogFormat::Logstash
        );
    }

    #[test]
    fn test_from_str_log4jxml() {
        assert_eq!(
            LogFormat::from_str("log4jxml").unwrap(),
            LogFormat::Log4jXML
        );
    }

    #[test]
    fn test_from_str_ndjson() {
        assert_eq!(LogFormat::from_str("ndjson").unwrap(), LogFormat::NDJSON);
    }

    #[test]
    fn test_from_str_mcp() {
        assert_eq!(LogFormat::from_str("mcp").unwrap(), LogFormat::MCP);
    }

    #[test]
    fn test_from_str_otlp() {
        assert_eq!(LogFormat::from_str("otlp").unwrap(), LogFormat::OTLP);
    }

    #[test]
    fn test_from_str_logfmt() {
        assert_eq!(LogFormat::from_str("logfmt").unwrap(), LogFormat::Logfmt);
    }

    #[test]
    fn test_from_str_ecs() {
        assert_eq!(LogFormat::from_str("ecs").unwrap(), LogFormat::ECS);
    }

    #[test]
    fn test_from_str_invalid() {
        assert!(LogFormat::from_str("invalid").is_err());
        let err = LogFormat::from_str("bogus").unwrap_err();
        assert!(err.to_string().contains("bogus"));
    }

    // ================================================================
    // Display – every variant
    // ================================================================

    #[test]
    fn test_display_clf() {
        assert_eq!(LogFormat::CLF.to_string(), "CLF");
    }

    #[test]
    fn test_display_json() {
        assert_eq!(LogFormat::JSON.to_string(), "JSON");
    }

    #[test]
    fn test_display_cef() {
        assert_eq!(LogFormat::CEF.to_string(), "CEF");
    }

    #[test]
    fn test_display_elf() {
        assert_eq!(LogFormat::ELF.to_string(), "ELF");
    }

    #[test]
    fn test_display_w3c() {
        assert_eq!(LogFormat::W3C.to_string(), "W3C");
    }

    #[test]
    fn test_display_gelf() {
        assert_eq!(LogFormat::GELF.to_string(), "GELF");
    }

    #[test]
    fn test_display_apache_access_log() {
        assert_eq!(LogFormat::ApacheAccessLog.to_string(), "Apache Access Log");
    }

    #[test]
    fn test_display_logstash() {
        assert_eq!(LogFormat::Logstash.to_string(), "Logstash");
    }

    #[test]
    fn test_display_log4j_xml() {
        assert_eq!(LogFormat::Log4jXML.to_string(), "Log4j XML");
    }

    #[test]
    fn test_display_ndjson() {
        assert_eq!(LogFormat::NDJSON.to_string(), "NDJSON");
    }

    #[test]
    fn test_display_mcp() {
        assert_eq!(LogFormat::MCP.to_string(), "MCP");
    }

    #[test]
    fn test_display_otlp() {
        assert_eq!(LogFormat::OTLP.to_string(), "OTLP");
    }

    #[test]
    fn test_display_logfmt() {
        assert_eq!(LogFormat::Logfmt.to_string(), "logfmt");
    }

    #[test]
    fn test_display_ecs() {
        assert_eq!(LogFormat::ECS.to_string(), "ECS");
    }

    // ================================================================
    // validate – all format types, valid and invalid
    // ================================================================

    // -- empty string always false --
    #[test]
    fn test_validate_empty_string_always_false() {
        let formats = [
            LogFormat::CLF,
            LogFormat::JSON,
            LogFormat::CEF,
            LogFormat::ELF,
            LogFormat::W3C,
            LogFormat::GELF,
            LogFormat::ApacheAccessLog,
            LogFormat::Logstash,
            LogFormat::Log4jXML,
            LogFormat::NDJSON,
            LogFormat::MCP,
            LogFormat::OTLP,
            LogFormat::Logfmt,
            LogFormat::ECS,
        ];
        for fmt in &formats {
            assert!(!fmt.validate(""), "{fmt} should reject empty strings");
        }
    }

    // -- CLF --
    #[test]
    fn test_validate_clf_valid() {
        let entry =
            r#"127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326"#;
        assert!(LogFormat::CLF.validate(entry));
    }

    #[test]
    fn test_validate_clf_invalid() {
        assert!(!LogFormat::CLF.validate("this is not CLF"));
    }

    // -- JSON --
    #[test]
    fn test_validate_json_valid() {
        assert!(LogFormat::JSON.validate(r#"{"key":"value"}"#));
    }

    #[test]
    fn test_validate_json_invalid() {
        assert!(!LogFormat::JSON.validate("{not json}"));
    }

    // -- CEF --
    #[test]
    fn test_validate_cef_valid() {
        let entry = "CEF:0|vendor|product|1.0|100|Test Event|5|key=value";
        assert!(LogFormat::CEF.validate(entry));
    }

    #[test]
    fn test_validate_cef_invalid() {
        assert!(!LogFormat::CEF.validate("not a CEF entry"));
    }

    // -- W3C --
    #[test]
    fn test_validate_w3c_valid() {
        let entry = "#Fields: date time s-ip\n2024-01-15 10:30:00 192.168.1.1";
        assert!(LogFormat::W3C.validate(entry));
    }

    #[test]
    fn test_validate_w3c_invalid() {
        assert!(!LogFormat::W3C.validate("no fields header"));
    }

    // -- GELF (JSON) --
    #[test]
    fn test_validate_gelf_valid() {
        let entry = r#"{"version":"1.1","host":"test","short_message":"hello"}"#;
        assert!(LogFormat::GELF.validate(entry));
    }

    #[test]
    fn test_validate_gelf_invalid() {
        assert!(!LogFormat::GELF.validate("not json"));
    }

    // -- Logstash (JSON) --
    #[test]
    fn test_validate_logstash_valid() {
        assert!(LogFormat::Logstash.validate(r#"{"@timestamp":"2024-01-01","message":"hi"}"#));
    }

    #[test]
    fn test_validate_logstash_invalid() {
        assert!(!LogFormat::Logstash.validate("{broken"));
    }

    // -- NDJSON (JSON) --
    #[test]
    fn test_validate_ndjson_valid() {
        assert!(LogFormat::NDJSON.validate(r#"{"a":1}"#));
    }

    #[test]
    fn test_validate_ndjson_invalid() {
        assert!(!LogFormat::NDJSON.validate("not json"));
    }

    // -- MCP (JSON) --
    #[test]
    fn test_validate_mcp_valid() {
        assert!(LogFormat::MCP.validate(r#"{"jsonrpc":"2.0"}"#));
    }

    #[test]
    fn test_validate_mcp_invalid() {
        assert!(!LogFormat::MCP.validate("{{bad}}"));
    }

    // -- OTLP (JSON) --
    #[test]
    fn test_validate_otlp_valid() {
        assert!(LogFormat::OTLP.validate(r#"{"body":"test"}"#));
    }

    #[test]
    fn test_validate_otlp_invalid() {
        assert!(!LogFormat::OTLP.validate("[nope"));
    }

    // -- ECS (JSON) --
    #[test]
    fn test_validate_ecs_valid() {
        assert!(LogFormat::ECS.validate(r#"{"@timestamp":"now"}"#));
    }

    #[test]
    fn test_validate_ecs_invalid() {
        assert!(!LogFormat::ECS.validate("plain text"));
    }

    // -- Logfmt --
    #[test]
    fn test_validate_logfmt_valid() {
        assert!(LogFormat::Logfmt.validate("level=info msg=\"hello\""));
    }

    #[test]
    fn test_validate_logfmt_invalid_no_equals() {
        assert!(!LogFormat::Logfmt.validate("no equals here"));
    }

    #[test]
    fn test_validate_logfmt_invalid_starts_with_equals() {
        assert!(!LogFormat::Logfmt.validate("=bad"));
    }

    // -- Log4jXML --
    #[test]
    fn test_validate_log4jxml_valid() {
        let entry = r#"<log4j:event logger="test" timestamp="123" level="INFO"><log4j:message>hi</log4j:message></log4j:event>"#;
        assert!(LogFormat::Log4jXML.validate(entry));
    }

    #[test]
    fn test_validate_log4jxml_invalid() {
        assert!(!LogFormat::Log4jXML.validate("just plain text"));
    }

    // -- ELF (always true for non-empty) --
    #[test]
    fn test_validate_elf_non_empty() {
        assert!(LogFormat::ELF.validate("anything"));
    }

    // -- ApacheAccessLog (always true for non-empty) --
    #[test]
    fn test_validate_apache_access_log_non_empty() {
        assert!(LogFormat::ApacheAccessLog.validate("anything"));
    }

    // ================================================================
    // format_log – passthrough, pretty-print, and error paths
    // ================================================================

    // -- CLF passthrough --
    #[test]
    fn test_format_log_clf_passthrough() {
        let entry = r#"127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] "GET / HTTP/1.0" 200 2326"#;
        let result = LogFormat::CLF.format_log(entry).unwrap();
        assert_eq!(result, entry);
    }

    // -- CEF passthrough --
    #[test]
    fn test_format_log_cef_passthrough() {
        let entry = "CEF:0|vendor|product|1.0|100|Name|5|";
        let result = LogFormat::CEF.format_log(entry).unwrap();
        assert_eq!(result, entry);
    }

    // -- ELF passthrough --
    #[test]
    fn test_format_log_elf_passthrough() {
        let entry = "ELF data here";
        let result = LogFormat::ELF.format_log(entry).unwrap();
        assert_eq!(result, entry);
    }

    // -- W3C passthrough --
    #[test]
    fn test_format_log_w3c_passthrough() {
        let entry = "W3C data here";
        let result = LogFormat::W3C.format_log(entry).unwrap();
        assert_eq!(result, entry);
    }

    // -- Log4jXML passthrough --
    #[test]
    fn test_format_log_log4jxml_passthrough() {
        let entry = "<log4j:event>data</log4j:event>";
        let result = LogFormat::Log4jXML.format_log(entry).unwrap();
        assert_eq!(result, entry);
    }

    // -- Logfmt passthrough --
    #[test]
    fn test_format_log_logfmt_passthrough() {
        let entry = "level=info msg=hello";
        let result = LogFormat::Logfmt.format_log(entry).unwrap();
        assert_eq!(result, entry);
    }

    // -- ApacheAccessLog passthrough --
    #[test]
    fn test_format_log_apache_passthrough() {
        let entry = "192.168.1.1 - - [date] \"GET / HTTP/1.1\" 200 0";
        let result = LogFormat::ApacheAccessLog.format_log(entry).unwrap();
        assert_eq!(result, entry);
    }

    // -- JSON pretty-prints --
    #[test]
    fn test_format_log_json_pretty_prints() {
        let entry = r#"{"key":"value"}"#;
        let result = LogFormat::JSON.format_log(entry).unwrap();
        // Pretty-printed JSON has newlines and indentation.
        assert!(result.contains('\n'));
        assert!(result.contains("  "));
        assert!(result.contains("\"key\""));
    }

    // -- JSON-based formats all pretty-print --
    #[test]
    fn test_format_log_logstash_pretty_prints() {
        let entry = r#"{"msg":"hi"}"#;
        let result = LogFormat::Logstash.format_log(entry).unwrap();
        assert!(result.contains('\n'));
    }

    #[test]
    fn test_format_log_ndjson_pretty_prints() {
        let entry = r#"{"a":1}"#;
        let result = LogFormat::NDJSON.format_log(entry).unwrap();
        assert!(result.contains('\n'));
    }

    #[test]
    fn test_format_log_gelf_pretty_prints() {
        let entry = r#"{"version":"1.1"}"#;
        let result = LogFormat::GELF.format_log(entry).unwrap();
        assert!(result.contains('\n'));
    }

    #[test]
    fn test_format_log_mcp_pretty_prints() {
        let entry = r#"{"jsonrpc":"2.0"}"#;
        let result = LogFormat::MCP.format_log(entry).unwrap();
        assert!(result.contains('\n'));
    }

    #[test]
    fn test_format_log_otlp_pretty_prints() {
        let entry = r#"{"body":"test"}"#;
        let result = LogFormat::OTLP.format_log(entry).unwrap();
        assert!(result.contains('\n'));
    }

    #[test]
    fn test_format_log_ecs_pretty_prints() {
        let entry = r#"{"@timestamp":"now"}"#;
        let result = LogFormat::ECS.format_log(entry).unwrap();
        assert!(result.contains('\n'));
    }

    // -- Invalid JSON produces an error for JSON-based formats --
    #[test]
    fn test_format_log_json_invalid_returns_error() {
        let result = LogFormat::JSON.format_log("{bad json}");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Invalid JSON"));
    }

    #[test]
    fn test_format_log_mcp_invalid_json_returns_error() {
        let result = LogFormat::MCP.format_log("not json at all");
        assert!(result.is_err());
    }

    // ================================================================
    // Serde / derive traits
    // ================================================================

    #[test]
    fn test_log_format_debug() {
        let dbg = format!("{:?}", LogFormat::JSON);
        assert_eq!(dbg, "JSON");
    }

    #[test]
    fn test_log_format_clone_copy() {
        let a = LogFormat::MCP;
        let b = a; // Copy
        let c = a; // Copy (LogFormat is Copy)
        assert_eq!(b, c);
    }

    #[test]
    fn test_log_format_eq_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(LogFormat::JSON);
        set.insert(LogFormat::JSON);
        assert_eq!(set.len(), 1);
        set.insert(LogFormat::MCP);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_log_format_serialize_deserialize() {
        let val = LogFormat::OTLP;
        let json = serde_json::to_string(&val).unwrap();
        let back: LogFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(val, back);
    }
}
