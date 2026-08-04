//! Structured diagnostics: subscriber setup, log format parsing, and URL redaction.
//!
//! See `openspec/changes/structured-diagnostics/` for the full design. This module
//! owns how AgentSync writes structured log events: always to stderr (never stdout),
//! in human or JSON format, at a level resolved from `--log-level` > `RUST_LOG` >
//! default `INFO`.

use std::fmt;
use std::io;
use std::io::IsTerminal;
use std::str::FromStr;

use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::fmt::format::FmtSpan;

/// Log event rendering format, selected via the global `--log-format` flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable, ANSI-colored lines (default).
    Human,
    /// Newline-delimited JSON events, stable for machine consumers.
    Json,
}

impl FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "human" => Ok(LogFormat::Human),
            "json" => Ok(LogFormat::Json),
            _ => Err(format!(
                "invalid log format '{s}' (expected 'human' or 'json')"
            )),
        }
    }
}

impl fmt::Display for LogFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogFormat::Human => f.write_str("human"),
            LogFormat::Json => f.write_str("json"),
        }
    }
}

/// Resolve the subscriber filter level by precedence:
/// explicit `--log-level` flag > `RUST_LOG` env var > default `INFO`.
///
/// `rust_log` is passed in explicitly (rather than read via `std::env`) so the
/// precedence logic stays pure and unit-testable without racing on process env.
pub fn resolve_level_filter(flag: Option<LevelFilter>, rust_log: Option<&str>) -> EnvFilter {
    if let Some(level) = flag {
        return EnvFilter::new(level.to_string());
    }
    if let Some(directives) = rust_log
        && let Ok(filter) = EnvFilter::try_new(directives)
    {
        return filter;
    }
    EnvFilter::new("info")
}

/// Strip credentials and query strings from a URL for safe inclusion in log events.
///
/// Removes `user:pass@` userinfo (up to and including the `@`) and drops anything
/// after `?` (query params may hold tokens). URLs without a scheme are returned
/// unchanged.
pub fn redact_url(url: &str) -> String {
    let mut redacted = url.to_string();
    if let Some(at) = redacted.find('@') {
        // Only strip userinfo when it is preceded by a scheme (`://`), so plain
        // "user@host" emails / usernames in paths are not mangled.
        let prefix = &redacted[..at];
        if let Some(scheme_end) = prefix.rfind("://") {
            redacted.replace_range(scheme_end + 3..at + 1, "");
        } else if url.starts_with("//") {
            redacted.replace_range(2..at + 1, "");
        }
    }
    if let Some(query_start) = redacted.find(['?', '#']) {
        redacted.truncate(query_start);
    }
    redacted
}

/// Install the global tracing subscriber.
///
/// Log events ALWAYS go to stderr so functional stdout (human output and the
/// machine-readable `--json` contracts) stays clean — this is the core fix for
/// issue #499. The filter level is resolved via [`resolve_level_filter`], and
/// `json` format additionally emits span-close events so span fields (agent id,
/// target, outcome) are observable by machine consumers.
pub fn init_logging(format: LogFormat, level: Option<LevelFilter>) {
    let filter = resolve_level_filter(level, std::env::var("RUST_LOG").ok().as_deref());
    match format {
        LogFormat::Human => tracing_subscriber::fmt()
            .with_writer(io::stderr)
            .with_ansi(use_ansi())
            .with_env_filter(filter)
            .init(),
        LogFormat::Json => tracing_subscriber::fmt()
            .json()
            .with_writer(io::stderr)
            .with_span_events(FmtSpan::CLOSE)
            .with_env_filter(filter)
            .init(),
    }
}

/// Whether human-format events may use ANSI escape codes.
///
/// Enabled only when stderr is a terminal (piped stderr stays plain, keeping
/// logs machine-parseable) and `NO_COLOR` is unset or empty.
fn use_ansi() -> bool {
    io::stderr().is_terminal() && std::env::var("NO_COLOR").map_or(true, |v| v.is_empty())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn log_format_from_str_human() {
        assert_eq!(LogFormat::from_str("human").unwrap(), LogFormat::Human);
    }

    #[test]
    fn log_format_from_str_json() {
        assert_eq!(LogFormat::from_str("json").unwrap(), LogFormat::Json);
    }

    #[test]
    fn log_format_from_str_case_insensitive() {
        assert_eq!(LogFormat::from_str("JSON").unwrap(), LogFormat::Json);
        assert_eq!(LogFormat::from_str("Human").unwrap(), LogFormat::Human);
    }

    #[test]
    fn log_format_from_str_whitespace_trimmed() {
        assert_eq!(LogFormat::from_str("  json ").unwrap(), LogFormat::Json);
    }

    #[test]
    fn log_format_from_str_invalid_rejected() {
        let err = LogFormat::from_str("xml").unwrap_err();
        assert!(err.contains("invalid log format"), "got: {err}");
        assert!(err.contains("'xml'"), "got: {err}");
        assert!(err.contains("human"), "got: {err}");
        assert!(err.contains("json"), "got: {err}");
    }

    #[test]
    fn log_format_from_str_empty_rejected() {
        assert!(LogFormat::from_str("").is_err());
    }

    #[test]
    fn log_format_display_roundtrip() {
        assert_eq!(LogFormat::Human.to_string(), "human");
        assert_eq!(LogFormat::Json.to_string(), "json");
    }

    #[test]
    fn resolve_level_filter_flag_beats_rust_log() {
        assert!(
            resolve_level_filter(Some(LevelFilter::DEBUG), Some("info"))
                .to_string()
                .contains("debug")
        );
    }

    #[test]
    fn resolve_level_filter_rust_log_when_no_flag() {
        assert!(
            resolve_level_filter(None, Some("warn"))
                .to_string()
                .contains("warn")
        );
    }

    #[test]
    fn resolve_level_filter_rust_log_off() {
        assert!(
            resolve_level_filter(None, Some("off"))
                .to_string()
                .contains("off")
        );
    }

    #[test]
    fn resolve_level_filter_preserves_target_directives() {
        assert!(
            resolve_level_filter(None, Some("agentsync::linker=debug"))
                .to_string()
                .contains("agentsync::linker=debug")
        );
    }

    #[test]
    fn resolve_level_filter_defaults_to_info() {
        assert!(
            resolve_level_filter(None, None)
                .to_string()
                .contains("info")
        );
    }

    #[test]
    fn redact_url_strips_userinfo() {
        assert_eq!(
            redact_url("https://user:secret@example.com/mcp"),
            "https://example.com/mcp"
        );
    }

    #[test]
    fn redact_url_strips_query_but_keeps_path() {
        assert_eq!(
            redact_url("https://example.com/stream?token=abc123"),
            "https://example.com/stream"
        );
    }

    #[test]
    fn redact_url_strips_both_userinfo_and_query() {
        assert_eq!(
            redact_url("https://user:pw@example.com/path?key=value"),
            "https://example.com/path"
        );
    }

    #[test]
    fn redact_url_leaves_plain_url_untouched() {
        assert_eq!(
            redact_url("https://example.com/health"),
            "https://example.com/health"
        );
    }

    #[test]
    fn redact_url_handles_url_without_scheme() {
        assert_eq!(redact_url("example.com/path"), "example.com/path");
    }

    #[test]
    fn redact_url_strips_scheme_relative_credentials_and_preserves_fragment() {
        assert_eq!(
            redact_url("//user:pass@example.com/path?token=x#frag"),
            "//example.com/path"
        );
        assert_eq!(
            redact_url("https://example.com/path#token"),
            "https://example.com/path"
        );
    }
}
