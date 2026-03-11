use crate::config::Config;
use crate::error::Result;

pub fn run() -> Result<()> {
    println!("mcpreg doctor — checking your setup\n");
    let mut issues = 0;

    // 1. Config directory
    match Config::config_dir() {
        Ok(dir) => {
            if dir.exists() {
                println!("  ✓ Config directory: {}", dir.display());
            } else {
                println!("  ⚠ Config directory not found: {}", dir.display());
                println!("    Run 'mcpreg config set registry_url <url>' to create it.");
                issues += 1;
            }
        }
        Err(e) => {
            println!("  ✗ Cannot determine config directory: {e}");
            issues += 1;
        }
    }

    // 2. Config file
    match Config::config_path() {
        Ok(path) => {
            if path.exists() {
                match Config::load() {
                    Ok(config) => {
                        println!("  ✓ Config file: {}", path.display());
                        println!("    Registry URL: {}", config.registry_url);
                        if config.api_key.is_some() {
                            println!("    API key: configured");
                        } else {
                            println!("    ⚠ API key: not set (required for publishing)");
                        }
                    }
                    Err(e) => {
                        println!("  ✗ Config file exists but failed to parse: {e}");
                        issues += 1;
                    }
                }
            } else {
                println!("  ℹ Config file not found (using defaults): {}", path.display());
            }
        }
        Err(e) => {
            println!("  ✗ Cannot determine config path: {e}");
            issues += 1;
        }
    }

    // 3. Claude Desktop config
    match Config::claude_desktop_config_path() {
        Ok(path) => {
            if path.exists() {
                println!("  ✓ Claude Desktop config: {}", path.display());
                // Check if it's valid JSON
                match std::fs::read_to_string(&path) {
                    Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                        Ok(config) => {
                            if let Some(servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
                                println!("    {} MCP server(s) configured", servers.len());
                            } else {
                                println!("    No mcpServers section found");
                            }
                        }
                        Err(e) => {
                            println!("  ✗ Claude config is not valid JSON: {e}");
                            issues += 1;
                        }
                    },
                    Err(e) => {
                        println!("  ✗ Cannot read Claude config: {e}");
                        issues += 1;
                    }
                }
            } else {
                println!("  ℹ Claude Desktop config not found: {}", path.display());
                println!("    Install Claude Desktop or create the file manually.");
            }
        }
        Err(e) => {
            println!("  ✗ Cannot determine Claude config path: {e}");
            issues += 1;
        }
    }

    // 4. Installed servers
    match Config::installed_servers_path() {
        Ok(path) => {
            if path.exists() {
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        match serde_json::from_str::<crate::api::types::InstalledServers>(&content) {
                            Ok(installed) => {
                                println!("  ✓ Installed servers: {} server(s)", installed.servers.len());
                            }
                            Err(e) => {
                                println!("  ✗ Installed servers file is corrupt: {e}");
                                issues += 1;
                            }
                        }
                    }
                    Err(e) => {
                        println!("  ✗ Cannot read installed servers: {e}");
                        issues += 1;
                    }
                }
            } else {
                println!("  ℹ No servers installed yet.");
            }
        }
        Err(e) => {
            println!("  ✗ Cannot determine installed servers path: {e}");
            issues += 1;
        }
    }

    // 5. Database
    match Config::db_path() {
        Ok(path) => {
            if path.exists() {
                match crate::registry::db::Database::open(&path.to_string_lossy()) {
                    Ok(db) => {
                        match db.stats() {
                            Ok(stats) => {
                                println!("  ✓ Registry database: {} servers, {} downloads",
                                    stats.total_servers, stats.total_downloads);
                            }
                            Err(e) => {
                                println!("  ✗ Database exists but stats query failed: {e}");
                                issues += 1;
                            }
                        }
                    }
                    Err(e) => {
                        println!("  ✗ Cannot open database: {e}");
                        issues += 1;
                    }
                }
            } else {
                println!("  ℹ Registry database not found (will be created by 'mcpreg serve')");
            }
        }
        Err(e) => {
            println!("  ✗ Cannot determine database path: {e}");
            issues += 1;
        }
    }

    // 6. Check common commands
    println!();
    for cmd in ["node", "npx", "uvx", "python3"] {
        let available = std::process::Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if available {
            println!("  ✓ {cmd} is available");
        } else {
            println!("  ⚠ {cmd} not found (some MCP servers need it)");
        }
    }

    println!();
    if issues == 0 {
        println!("All checks passed! ✓");
    } else {
        println!("{issues} issue(s) found. See above for details.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doctor_does_not_panic() {
        // doctor should never panic, only report
        let _ = run();
    }
}
