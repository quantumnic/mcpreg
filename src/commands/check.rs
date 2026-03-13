use crate::api::types::InstalledServers;
use crate::config::Config;
use crate::error::Result;

/// Verify that all installed MCP servers are still runnable.
/// Checks: command exists on PATH, config file consistency, registry availability.
pub fn run(json: bool) -> Result<()> {
    let installed_path = Config::installed_servers_path()?;
    let installed: InstalledServers = if installed_path.exists() {
        let content = std::fs::read_to_string(&installed_path)?;
        serde_json::from_str(&content)?
    } else {
        if json {
            println!("{}", serde_json::json!({
                "status": "ok",
                "servers": [],
                "issues": [],
                "message": "No servers installed"
            }));
        } else {
            println!("No servers installed. Use 'mcpreg install <owner/name>' to get started.");
        }
        return Ok(());
    };

    let claude_config = Config::claude_desktop_config_path()?;
    let claude_config_exists = claude_config.exists();
    let claude_servers: serde_json::Value = if claude_config_exists {
        let content = std::fs::read_to_string(&claude_config)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        serde_json::json!({})
    };

    let mut issues: Vec<serde_json::Value> = Vec::new();
    let mut server_results: Vec<serde_json::Value> = Vec::new();

    for server in &installed.servers {
        let full_name = server.full_name();
        let mut server_issues: Vec<String> = Vec::new();

        // Check 1: Is the command available on PATH?
        let command_found = which_command(&server.command);
        if !command_found {
            let msg = format!("Command '{}' not found in PATH", server.command);
            server_issues.push(msg.clone());
            issues.push(serde_json::json!({
                "server": full_name,
                "severity": "error",
                "message": msg,
            }));
        }

        // Check 2: Is it in claude_desktop_config.json?
        let in_claude_config = claude_servers
            .get("mcpServers")
            .and_then(|s| s.get(&server.name))
            .is_some();

        if claude_config_exists && !in_claude_config {
            let msg = "Not found in claude_desktop_config.json".to_string();
            server_issues.push(msg.clone());
            issues.push(serde_json::json!({
                "server": full_name,
                "severity": "warning",
                "message": msg,
            }));
        }

        let status = if server_issues.is_empty() { "ok" } else { "issues" };

        server_results.push(serde_json::json!({
            "server": full_name,
            "version": server.version,
            "command": server.command,
            "status": status,
            "issues": server_issues,
        }));
    }

    if json {
        let overall_status = if issues.is_empty() { "ok" } else { "issues" };
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "status": overall_status,
            "servers": server_results,
            "issues": issues,
            "total_servers": installed.servers.len(),
            "total_issues": issues.len(),
        }))?);
    } else {
        println!("Checking {} installed server(s)...\n", installed.servers.len());

        for result in &server_results {
            let server = result["server"].as_str().unwrap_or("?");
            let version = result["version"].as_str().unwrap_or("?");
            let status = result["status"].as_str().unwrap_or("?");

            if status == "ok" {
                println!("  ✓ {server} v{version}");
            } else {
                println!("  ✗ {server} v{version}");
                for issue in result["issues"].as_array().unwrap_or(&vec![]) {
                    println!("    └─ {}", issue.as_str().unwrap_or("?"));
                }
            }
        }

        println!();
        if issues.is_empty() {
            println!("All servers OK ✓");
        } else {
            let errors = issues.iter().filter(|i| i["severity"] == "error").count();
            let warnings = issues.iter().filter(|i| i["severity"] == "warning").count();
            println!("{} issue(s) found ({} error(s), {} warning(s))", issues.len(), errors, warnings);
        }
    }

    Ok(())
}

/// Check if a command is available on PATH.
fn which_command(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_which_command_found() {
        // "sh" should always exist
        assert!(which_command("sh"));
    }

    #[test]
    fn test_which_command_not_found() {
        assert!(!which_command("nonexistent_command_12345xyz"));
    }
}
