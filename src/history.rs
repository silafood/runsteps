//! History storage for `--again` replay (US-007).
//!
//! History is persisted at `dirs::cache_dir()/runsteps/history.json`.
//! Writes are atomic (write to temp file, then rename).
//! At most 10 entries are retained per config path (oldest evicted).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A single replay entry recording which steps were run for a given config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Absolute path to the config file used.
    pub config_path: String,
    /// SHA-256 hex digest of the config file bytes at the time of the run.
    pub config_sha256: String,
    /// Ordered list of step names that were selected and executed.
    pub selected: Vec<String>,
    /// RFC3339 timestamp of the run.
    pub timestamp: String,
}

/// The top-level history file structure.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct History {
    pub version: u32,
    pub entries: Vec<HistoryEntry>,
}

const MAX_ENTRIES_PER_CONFIG: usize = 10;

/// Compute SHA-256 hex digest of arbitrary bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Return the path to the history file.
///
/// Checks `RUNSTEPS_CACHE_DIR` first (allows tests and users to override),
/// then falls back to `dirs::cache_dir()`.
fn history_path() -> Option<std::path::PathBuf> {
    if let Ok(override_dir) = std::env::var("RUNSTEPS_CACHE_DIR") {
        return Some(std::path::PathBuf::from(override_dir).join("history.json"));
    }
    dirs::cache_dir().map(|d| d.join("runsteps").join("history.json"))
}

/// Load the history file, returning an empty History on any read/parse failure.
pub fn load_history() -> History {
    let path = match history_path() {
        Some(p) => p,
        None => return History::default(),
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return History::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

/// Append or update an entry for the given config path, then save atomically.
/// Retains at most `MAX_ENTRIES_PER_CONFIG` entries per config path (oldest evicted).
pub fn save_history_entry(entry: HistoryEntry) -> Result<()> {
    let path = match history_path() {
        Some(p) => p,
        None => anyhow::bail!("cannot determine cache directory"),
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut history = load_history();
    history.version = 1;

    // Collect indices for this config path, oldest first.
    let same_indices: Vec<usize> = history
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.config_path == entry.config_path)
        .map(|(i, _)| i)
        .collect();

    // Evict oldest entries if at the limit (make room for the new one).
    if same_indices.len() >= MAX_ENTRIES_PER_CONFIG {
        let to_drop = same_indices.len() - (MAX_ENTRIES_PER_CONFIG - 1);
        let drop_set: std::collections::HashSet<usize> =
            same_indices.into_iter().take(to_drop).collect();
        let mut idx = 0usize;
        history.entries.retain(|_| {
            let keep = !drop_set.contains(&idx);
            idx += 1;
            keep
        });
    }

    history.entries.push(entry);

    // Atomic write: write to a temp file next to the target, then rename.
    let tmp_path = path.with_extension("tmp");
    let json = serde_json::to_string_pretty(&history)?;
    std::fs::write(&tmp_path, json)?;
    std::fs::rename(&tmp_path, &path)?;

    Ok(())
}

/// Find the most recent entry for the given config path.
pub fn latest_entry_for(config_path: &str) -> Option<HistoryEntry> {
    let history = load_history();
    history
        .entries
        .into_iter()
        .rfind(|e| e.config_path == config_path)
}

/// Return an RFC3339 timestamp string for now (UTC, second precision).
pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let (year, month, day, hour, minute, second) = unix_secs_to_datetime(secs);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

/// Convert UNIX seconds to (year, month, day, hour, minute, second) UTC.
fn unix_secs_to_datetime(mut secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let second = secs % 60;
    secs /= 60;
    let minute = secs % 60;
    secs /= 60;
    let hour = secs % 24;
    secs /= 24;

    // Days since epoch → calendar date (Gregorian).
    let mut days = secs;
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let month_days: [u64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    let day = days + 1;
    (year, month, day, hour, minute, second)
}

fn is_leap(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}
