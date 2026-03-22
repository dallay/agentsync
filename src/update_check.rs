use std::fs;
use std::path::PathBuf;
use std::thread;

use anyhow::Context;
use colored::Colorize;
use is_terminal::IsTerminal;
use semver::Version;
use serde::{Deserialize, Serialize};

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
    fn load(&self) -> Option<CheckedVersion> {
        let data = fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&data).ok()
    }

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

pub fn spawn() {
    let no_check = std::env::var("AGENTSYNC_NO_UPDATE_CHECK")
        .map(|v| v.eq_ignore_ascii_case("1"))
        .unwrap_or(false);
    if no_check {
        return;
    }

    let ci = std::env::var("CI")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if ci {
        return;
    }

    if !std::io::stderr().is_terminal() {
        return;
    }

    let _ = thread::Builder::new()
        .name("agentsync-update-check".to_string())
        .spawn(|| {
            let path = cache_path();
            let cache = Cache { path };

            if cache.load().is_some_and(|c| is_fresh(&c)) {
                return;
            }

            let client = match reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(3))
                .build()
            {
                Ok(c) => c,
                Err(_) => return,
            };

            let response = match client.get(CRATES_IO_URL).send() {
                Ok(r) => r,
                Err(_) => return,
            };

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

            let info: CratesIoResponse = match response.json() {
                Ok(v) => v,
                Err(_) => return,
            };

            let current = match Version::parse(env!("CARGO_PKG_VERSION")) {
                Ok(v) => v,
                Err(_) => return,
            };

            let latest = match Version::parse(&info.krate.newest_version) {
                Ok(v) => v,
                Err(_) => return,
            };

            if !latest.pre.is_empty() {
                return;
            }

            if latest <= current {
                return;
            }

            let new_cache = CheckedVersion {
                last_checked: chrono::Utc::now().timestamp(),
                latest_version: info.krate.newest_version.clone(),
                notified_for_version: Some(info.krate.newest_version.clone()),
            };

            if cache.save(&new_cache).is_err() {
                return;
            }

            eprintln!(
                "{} {}",
                "💡".yellow().bold(),
                format!(
                    "A new version of agentsync is available: {} (you have {}). Run cargo install agentsync to update.",
                    info.krate.newest_version.yellow().bold(),
                    env!("CARGO_PKG_VERSION").dimmed()
                )
                .yellow()
                .bold()
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

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
}
