use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

/// Show cache statistics or clear the local cache.
pub fn run(action: Option<&str>, json_output: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    match action.unwrap_or("status") {
        "status" | "info" => show_status(&db_path, json_output),
        "clear" | "reset" => clear_cache(&db_path, json_output),
        "path" => {
            println!("{db_path}");
            Ok(())
        }
        other => {
            eprintln!("Unknown cache action: '{other}'");
            eprintln!("Valid actions: status, clear, path");
            Ok(())
        }
    }
}

fn show_status(db_path: &str, json_output: bool) -> Result<()> {
    let path = std::path::Path::new(db_path);
    let exists = path.exists();
    let size_bytes = if exists {
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    let server_count = if exists {
        match Database::open(db_path) {
            Ok(db) => db.count_servers().unwrap_or(0),
            Err(_) => 0,
        }
    } else {
        0
    };

    if json_output {
        let info = serde_json::json!({
            "path": db_path,
            "exists": exists,
            "size_bytes": size_bytes,
            "size_human": format_size(size_bytes),
            "server_count": server_count,
        });
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("Cache status:");
        println!("  Path:    {db_path}");
        println!("  Exists:  {exists}");
        println!("  Size:    {}", format_size(size_bytes));
        println!("  Servers: {server_count}");
    }
    Ok(())
}

fn clear_cache(db_path: &str, json_output: bool) -> Result<()> {
    let path = std::path::Path::new(db_path);
    if path.exists() {
        std::fs::remove_file(path)?;
        if json_output {
            println!(r#"{{"cleared": true, "path": "{}"}}"#, db_path);
        } else {
            println!("✓ Cache cleared: {db_path}");
        }
    } else if json_output {
        println!(r#"{{"cleared": false, "reason": "no cache file"}}"#);
    } else {
        println!("No cache file to clear.");
    }
    Ok(())
}

fn format_size(bytes: u64) -> String {
    match bytes {
        b if b >= 1_048_576 => format!("{:.1} MB", b as f64 / 1_048_576.0),
        b if b >= 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{b} B"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1_048_576), "1.0 MB");
        assert_eq!(format_size(2_621_440), "2.5 MB");
    }

    #[test]
    fn test_cache_status_nonexistent_path() {
        // Should not panic on nonexistent path
        let result = run(Some("status"), true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cache_path_action() {
        let result = run(Some("path"), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cache_unknown_action() {
        let result = run(Some("bogus"), false);
        assert!(result.is_ok());
    }
}
