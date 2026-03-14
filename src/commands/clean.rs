use crate::config;
use crate::error::Result;

/// Clean up stale mcpreg data (cache, old backups, orphaned files).
pub fn run_clean(dry_run: bool) -> Result<()> {
    let config_dir = config::Config::config_dir()?;
    let mut cleaned = 0u64;
    let mut total_bytes = 0u64;

    // Clean old backup files (*.bak, *.old)
    let patterns = ["*.bak", "*.old", "*.tmp"];
    for pattern in &patterns {
        let glob_path = config_dir.join(pattern);
        if let Some(glob_str) = glob_path.to_str() {
            // Manual walk since we don't have the glob crate
            if let Ok(entries) = std::fs::read_dir(&config_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        let matches = match *pattern {
                            "*.bak" => ext == "bak",
                            "*.old" => ext == "old",
                            "*.tmp" => ext == "tmp",
                            _ => false,
                        };
                        if matches {
                            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            if dry_run {
                                println!("  Would remove: {} ({} bytes)", path.display(), size);
                            } else if let Err(e) = std::fs::remove_file(&path) {
                                eprintln!("  Failed to remove {}: {e}", path.display());
                            } else {
                                println!("  Removed: {}", path.display());
                            }
                            cleaned += 1;
                            total_bytes += size;
                        }
                    }
                }
            }
            let _ = glob_str; // suppress unused warning
        }
    }

    // Clean empty directories in config_dir
    if let Ok(entries) = std::fs::read_dir(&config_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(mut dir) = std::fs::read_dir(&path) {
                    if dir.next().is_none() {
                        if dry_run {
                            println!("  Would remove empty dir: {}", path.display());
                        } else if let Err(e) = std::fs::remove_dir(&path) {
                            eprintln!("  Failed to remove dir {}: {e}", path.display());
                        } else {
                            println!("  Removed empty dir: {}", path.display());
                        }
                        cleaned += 1;
                    }
                }
            }
        }
    }

    if cleaned == 0 {
        println!("Nothing to clean up. ✨");
    } else {
        let action = if dry_run { "Would clean" } else { "Cleaned" };
        println!("{action} {cleaned} item(s), freeing ~{} bytes.", total_bytes);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_dry_run_no_crash() {
        // Just verify it doesn't panic in dry-run mode
        let result = run_clean(true);
        assert!(result.is_ok());
    }
}
