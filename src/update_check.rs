use std::fs;
use std::path::PathBuf;
use std::thread;

use anyhow::Context;
use is_terminal::IsTerminal;
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum UpdateCheckError {
    #[error("update check timed out after {duration_secs}s for url {url}")]
    Timeout { url: String, duration_secs: u64 },
    #[error("connection failed for {url}: {reason}")]
    Connection { url: String, reason: String },
    #[error("unexpected HTTP status {status} for {url}")]
    HttpStatus { url: String, status: u16 },
    #[error("failed to parse version: {0}")]
    ParseError(String),
}

const CACHE_TTL_SECS: i64 = 24 * 60 * 60;
const CRATES_IO_URL: &str = "https://crates.io/api/v1/crates/agentsync";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckedVersion {
    #[serde(rename = "last_checked")]
    last_checked: i64,
    #[serde(rename = "latest_version")]
    latest_version: String,
    #[serde(rename = "notified_for_version")]
    notified_for_version: Option<String>,
}

#[derive(Debug, Clone)]
struct Cache {
    path: PathBuf,
}

impl Cache {
    /// Note: sync path — file I/O on small JSON cache; blocking is fast and appropriate.
    fn load(&self) -> Option<CheckedVersion> {
        let data = fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Note: sync path — file I/O on small JSON cache; blocking is fast and appropriate.
    fn save(&self, v: &CheckedVersion) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).context("failed to create cache directory")?;
        }
        let data = serde_json::to_string_pretty(v).context("failed to serialize cache")?;
        fs::write(&self.path, data).context("failed to write cache file")?;
        Ok(())
    }
}

fn cache_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".cache")
        .join("agentsync")
        .join("update-check.json")
}

fn is_fresh(cache: &CheckedVersion) -> bool {
    let now = chrono::Utc::now().timestamp();
    if now - cache.last_checked > CACHE_TTL_SECS {
        return false;
    }
    if cache.notified_for_version.as_ref() != Some(&cache.latest_version) {
        return false;
    }
    true
}

/// Pure logic for determining whether to skip the update check, given the
/// relevant environment values and terminal state.
fn should_skip(no_update_check: Option<&str>, ci: Option<&str>, is_terminal: bool) -> bool {
    if no_update_check.is_some_and(|v| v.eq_ignore_ascii_case("1")) {
        return true;
    }
    if ci.is_some_and(|v| v.eq_ignore_ascii_case("true")) {
        return true;
    }
    !is_terminal
}

fn should_skip_update_check() -> bool {
    let no_check = std::env::var("AGENTSYNC_NO_UPDATE_CHECK").ok();
    let ci = std::env::var("CI").ok();
    let is_terminal = std::io::stderr().is_terminal();
    should_skip(no_check.as_deref(), ci.as_deref(), is_terminal)
}

async fn fetch_latest_version_async(
    url: &str,
    timeout: std::time::Duration,
) -> Result<String, UpdateCheckError> {
    #[derive(Deserialize)]
    struct CratesIoResponse {
        #[serde(rename = "crate")]
        krate: CrateInfo,
    }

    #[derive(Deserialize)]
    struct CrateInfo {
        #[serde(rename = "newest_version")]
        newest_version: String,
    }

    let client = reqwest::Client::builder()
        .user_agent(concat!("agentsync/", env!("CARGO_PKG_VERSION")))
        .timeout(timeout)
        .build()
        .map_err(|e| UpdateCheckError::Connection {
            url: url.to_string(),
            reason: e.to_string(),
        })?;

    let response = client.get(url).send().await.map_err(|e| {
        if e.is_timeout() {
            UpdateCheckError::Timeout {
                url: url.to_string(),
                duration_secs: timeout.as_secs(),
            }
        } else {
            UpdateCheckError::Connection {
                url: url.to_string(),
                reason: e.to_string(),
            }
        }
    })?;

    let status = response.status();
    if !status.is_success() {
        return Err(UpdateCheckError::HttpStatus {
            url: url.to_string(),
            status: status.as_u16(),
        });
    }

    let info: CratesIoResponse = response.json().await.map_err(|e| {
        if e.is_timeout() {
            UpdateCheckError::Timeout {
                url: url.to_string(),
                duration_secs: timeout.as_secs(),
            }
        } else {
            UpdateCheckError::ParseError(e.to_string())
        }
    })?;

    Ok(info.krate.newest_version)
}

async fn check_and_notify_async() {
    let cache = Cache { path: cache_path() };

    // Note: sync path — file I/O on small JSON cache; blocking is fast and appropriate.
    if cache.load().is_some_and(|c| is_fresh(&c)) {
        return;
    }

    let newest_version =
        match fetch_latest_version_async(CRATES_IO_URL, std::time::Duration::from_secs(3)).await {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(?e, "update check failed");
                return;
            }
        };

    let Ok(current) = Version::parse(env!("CARGO_PKG_VERSION")) else {
        return;
    };

    let Ok(latest) = Version::parse(&newest_version) else {
        return;
    };

    if !latest.pre.is_empty() || latest <= current {
        return;
    }

    let new_cache = CheckedVersion {
        last_checked: chrono::Utc::now().timestamp(),
        latest_version: newest_version.clone(),
        notified_for_version: Some(newest_version.clone()),
    };

    info!(
        latest_version = %newest_version,
        current_version = env!("CARGO_PKG_VERSION"),
        "A new version of agentsync is available; run cargo install agentsync to update"
    );

    // Note: sync path — file I/O on small JSON cache; blocking is fast and appropriate.
    let _ = cache.save(&new_cache);
}

pub fn spawn() {
    if should_skip_update_check() {
        return;
    }

    let _ = thread::Builder::new()
        .name("agentsync-update-check".to_string())
        .spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(check_and_notify_async());
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Start a TCP server that delays its response by `delay_secs`.
    async fn spawn_delayed_server(delay_secs: u64) -> std::net::SocketAddr {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let _handle = std::thread::spawn(move || {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind to succeed");
            let addr = listener.local_addr().expect("local_addr");
            if ready_tx.send(addr).is_err() {
                return;
            }
            let (mut conn, _) = listener.accept().expect("accept");
            std::thread::sleep(std::time::Duration::from_secs(delay_secs));
            use std::io::{Read, Write};
            // Write response then read request (drain it) before closing
            let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello");
            let _ = conn.flush();
            // Read to EOF to keep conn open until client is done
            let mut dummy = [0u8; 256];
            let _ = conn.read(&mut dummy);
        });
        ready_rx.await.expect("addr received")
    }

    /// Start a TCP server that returns non-JSON.
    async fn spawn_non_json_server() -> std::net::SocketAddr {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let _handle = std::thread::spawn(move || {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind to succeed");
            let addr = listener.local_addr().expect("local_addr");
            if ready_tx.send(addr).is_err() {
                return;
            }
            let (mut conn, _) = listener.accept().expect("accept");
            use std::io::{Read, Write};
            // Read HTTP request to drain it, then send our response
            let mut dummy = [0u8; 512];
            let _ = conn.read(&mut dummy);
            let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nnot json!!!");
            let _ = conn.flush();
        });
        ready_rx.await.expect("addr received")
    }

    /// Start a TCP server that returns a valid crates.io JSON body.
    async fn spawn_success_server() -> std::net::SocketAddr {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let _handle = std::thread::spawn(move || {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind to succeed");
            let addr = listener.local_addr().expect("local_addr");
            if ready_tx.send(addr).is_err() {
                return;
            }
            let (mut conn, _) = listener.accept().expect("accept");
            use std::io::{Read, Write};
            let mut dummy = [0u8; 512];
            let _ = conn.read(&mut dummy);
            let body = br#"{"crate":{"newest_version":"9.9.9"}}"#;
            let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n");
            let _ = write!(conn, "Content-Length: {}\r\n\r\n", body.len());
            let _ = conn.write_all(body);
            let _ = conn.flush();
        });
        ready_rx.await.expect("addr received")
    }

    /// Start a TCP server that sends response headers but stalls the body,
    /// so the client timeout fires while reading the response body.
    async fn spawn_body_stall_server() -> std::net::SocketAddr {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let _handle = std::thread::spawn(move || {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind to succeed");
            let addr = listener.local_addr().expect("local_addr");
            if ready_tx.send(addr).is_err() {
                return;
            }
            let (mut conn, _) = listener.accept().expect("accept");
            use std::io::{Read, Write};
            let mut dummy = [0u8; 512];
            let _ = conn.read(&mut dummy);
            // Advertise a large body, then never send it and keep the socket open.
            let _ = conn.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 65536\r\n\r\n",
            );
            let _ = conn.flush();
            // Keep the connection open long enough for the client timeout to fire.
            std::thread::sleep(std::time::Duration::from_secs(5));
        });
        ready_rx.await.expect("addr received")
    }

    /// Start a TCP server that returns HTTP 404.
    async fn spawn_404_server() -> std::net::SocketAddr {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let _handle = std::thread::spawn(move || {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind to succeed");
            let addr = listener.local_addr().expect("local_addr");
            if ready_tx.send(addr).is_err() {
                return;
            }
            let (mut conn, _) = listener.accept().expect("accept");
            use std::io::{Read, Write};
            // Read HTTP request to drain it, then send 404
            let mut dummy = [0u8; 512];
            let _ = conn.read(&mut dummy);
            let _ = conn.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            let _ = conn.flush();
        });
        ready_rx.await.expect("addr received")
    }

    #[tokio::test]
    async fn test_fetch_latest_version_timeout() {
        let addr = spawn_delayed_server(5).await;
        let url = format!("http://{}/api/v1/crates/agentsync", addr);

        let result = fetch_latest_version_async(&url, std::time::Duration::from_millis(50)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, UpdateCheckError::Timeout { .. }));
    }

    #[tokio::test]
    async fn test_fetch_latest_version_invalid_json() {
        let addr = spawn_non_json_server().await;
        let url = format!("http://{}/api/v1/crates/agentsync", addr);

        let result = fetch_latest_version_async(&url, std::time::Duration::from_secs(5)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, UpdateCheckError::ParseError(_)));
    }

    #[tokio::test]
    async fn test_fetch_latest_version_success() {
        let addr = spawn_success_server().await;
        let url = format!("http://{}/api/v1/crates/agentsync", addr);

        let result = fetch_latest_version_async(&url, std::time::Duration::from_secs(5)).await;
        assert_eq!(result.expect("successful fetch"), "9.9.9");
    }

    #[tokio::test]
    async fn test_fetch_latest_version_body_stall_timeout() {
        let addr = spawn_body_stall_server().await;
        let url = format!("http://{}/api/v1/crates/agentsync", addr);

        // Headers arrive quickly, but the body never arrives. The configured client
        // timeout must surface as UpdateCheckError::Timeout, not ParseError.
        let result = fetch_latest_version_async(&url, std::time::Duration::from_millis(100)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, UpdateCheckError::Timeout { .. }),
            "expected Timeout for stalled body, got: {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_fetch_latest_version_404() {
        let addr = spawn_404_server().await;
        let url = format!("http://{}/api/v1/crates/agentsync", addr);

        let result = fetch_latest_version_async(&url, std::time::Duration::from_secs(5)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            UpdateCheckError::HttpStatus { status: 404, .. }
        ));
    }

    #[test]
    fn test_cache_load_nonexistent() {
        let cache = Cache {
            path: PathBuf::from("/nonexistent/path/cache.json"),
        };
        assert!(cache.load().is_none());
    }

    #[test]
    fn test_cache_load_corrupted_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("cache.json");
        fs::write(&path, "not valid json").unwrap();
        let cache = Cache { path };
        assert!(cache.load().is_none());
    }

    #[test]
    fn test_cache_load_missing_fields() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("cache.json");
        fs::write(&path, r#"{"last_checked": 123}"#).unwrap();
        let cache = Cache { path };
        assert!(cache.load().is_none());
    }

    #[test]
    fn test_cache_save_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("subdir").join("nested").join("cache.json");
        let cache = Cache { path };
        let data = CheckedVersion {
            last_checked: 123,
            latest_version: "1.0.0".to_string(),
            notified_for_version: Some("1.0.0".to_string()),
        };
        assert!(cache.save(&data).is_ok());
        assert!(
            fs::read_to_string(tmp.path().join("subdir").join("nested").join("cache.json")).is_ok()
        );
    }

    #[test]
    fn test_cache_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("cache.json");
        let cache = Cache { path };
        let data = CheckedVersion {
            last_checked: 999,
            latest_version: "2.0.0".to_string(),
            notified_for_version: Some("2.0.0".to_string()),
        };
        cache.save(&data).unwrap();
        let loaded = cache.load().unwrap();
        assert_eq!(loaded.last_checked, 999);
        assert_eq!(loaded.latest_version, "2.0.0");
        assert_eq!(loaded.notified_for_version, Some("2.0.0".to_string()));
    }

    #[test]
    fn test_version_skip_prerelease() {
        let latest = Version::parse("1.1.0-beta.1").unwrap();
        assert!(!latest.pre.is_empty());
    }

    #[test]
    fn test_version_detects_newer() {
        let current = Version::parse("1.0.0").unwrap();
        let latest = Version::parse("1.1.0").unwrap();
        assert!(latest.pre.is_empty());
        assert!(latest > current);
    }

    #[test]
    fn test_version_current_newer_or_equal() {
        let current = Version::parse("2.0.0").unwrap();
        let latest = Version::parse("1.1.0").unwrap();
        assert!(latest <= current);

        let current_eq = Version::parse("1.1.0").unwrap();
        let latest_eq = Version::parse("1.1.0").unwrap();
        assert!(latest_eq <= current_eq);
    }

    #[test]
    fn test_cache_fresh_if_notified_matches_latest() {
        let now = chrono::Utc::now().timestamp();
        let cache = CheckedVersion {
            last_checked: now,
            latest_version: "1.0.0".to_string(),
            notified_for_version: Some("1.0.0".to_string()),
        };
        assert!(is_fresh(&cache));
    }

    #[test]
    fn test_cache_not_fresh_if_stale() {
        let stale_time = chrono::Utc::now().timestamp() - (CACHE_TTL_SECS + 1);
        let cache = CheckedVersion {
            last_checked: stale_time,
            latest_version: "1.0.0".to_string(),
            notified_for_version: Some("1.0.0".to_string()),
        };
        assert!(!is_fresh(&cache));
    }

    #[test]
    fn test_cache_not_fresh_if_notified_differs() {
        let cache = CheckedVersion {
            last_checked: chrono::Utc::now().timestamp(),
            latest_version: "2.0.0".to_string(),
            notified_for_version: Some("1.0.0".to_string()),
        };
        assert!(!is_fresh(&cache));
    }

    #[test]
    fn test_cache_not_fresh_if_notified_is_none() {
        let cache = CheckedVersion {
            last_checked: chrono::Utc::now().timestamp(),
            latest_version: "1.0.0".to_string(),
            notified_for_version: None,
        };
        assert!(!is_fresh(&cache));
    }

    #[test]
    fn test_should_skip_when_no_update_check_set() {
        assert!(should_skip(Some("1"), None, true));
    }

    #[test]
    fn test_should_skip_when_ci_set() {
        assert!(should_skip(None, Some("true"), true));
    }

    #[test]
    fn test_should_skip_no_update_check_only_skips_on_1() {
        // "0" should not trigger skip (terminal=true means not skipped)
        assert!(!should_skip(Some("0"), None, true));
    }

    #[test]
    fn test_should_skip_when_not_terminal() {
        assert!(should_skip(None, None, false));
    }

    #[test]
    fn test_should_not_skip_when_all_clear() {
        assert!(!should_skip(None, None, true));
    }
}
