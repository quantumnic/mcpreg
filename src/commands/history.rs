use crate::config::Config;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single history entry tracking a user action.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HistoryEntry {
    pub action: String,
    pub server: Option<String>,
    pub timestamp: String,
    pub details: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
}

impl History {
    fn path() -> Result<PathBuf> {
        Ok(Config::config_dir()?.join("history.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let history: History = serde_json::from_str(&content).unwrap_or_default();
            Ok(history)
        } else {
            Ok(History::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Record an action. Keeps the last 100 entries.
    pub fn record(action: &str, server: Option<&str>, details: Option<&str>) -> Result<()> {
        let mut history = Self::load()?;
        let entry = HistoryEntry {
            action: action.to_string(),
            server: server.map(String::from),
            timestamp: chrono_now(),
            details: details.map(String::from),
        };
        history.entries.push(entry);
        // Keep last 100
        if history.entries.len() > 100 {
            let start = history.entries.len() - 100;
            history.entries = history.entries[start..].to_vec();
        }
        history.save()?;
        Ok(())
    }
}

/// Simple ISO-8601-ish timestamp without pulling in chrono.
fn chrono_now() -> String {
    // Use system time formatted as RFC-3339-like
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    // Simple formatting: days since epoch
    let days = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

    // Calculate year/month/day from days since epoch (1970-01-01)
    let (year, month, day) = days_to_date(days);

    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

fn days_to_date(days: u64) -> (u64, u64, u64) {
    //算法: https://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

pub fn run(limit: usize, json_output: bool) -> Result<()> {
    let history = History::load()?;

    if json_output {
        let entries: Vec<_> = history.entries.iter().rev().take(limit).collect();
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    if history.entries.is_empty() {
        println!("No history yet. Install, update, or search for servers to build history.");
        return Ok(());
    }

    let entries: Vec<_> = history.entries.iter().rev().take(limit).collect();
    println!("Recent actions ({} of {}):\n", entries.len(), history.entries.len());

    for entry in &entries {
        let server_str = entry.server.as_deref().unwrap_or("");
        let detail_str = entry.details.as_deref().map(|d| format!(" — {d}")).unwrap_or_default();
        println!("  {} {} {}{}", entry.timestamp, entry.action, server_str, detail_str);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrono_now_format() {
        let ts = chrono_now();
        // Should look like YYYY-MM-DDTHH:MM:SSZ
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.len(), 20);
    }

    #[test]
    fn test_days_to_date_epoch() {
        let (y, m, d) = days_to_date(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_days_to_date_known() {
        // 2024-01-01 is day 19723
        let (y, m, d) = days_to_date(19723);
        assert_eq!((y, m, d), (2024, 1, 1));
    }

    #[test]
    fn test_history_default_empty() {
        let h = History::default();
        assert!(h.entries.is_empty());
    }

    #[test]
    fn test_history_entry_serde_roundtrip() {
        let entry = HistoryEntry {
            action: "install".into(),
            server: Some("alice/tool".into()),
            timestamp: "2024-01-01T00:00:00Z".into(),
            details: Some("v1.0.0".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.action, "install");
        assert_eq!(back.server.unwrap(), "alice/tool");
    }
}
