use crate::api::types::McpManifest;
use crate::error::{McpRegError, Result};
use std::path::Path;

pub fn run(manifest_path: Option<&str>, json_output: bool) -> Result<()> {
    let path = manifest_path.unwrap_or("mcpreg.toml");
    let path = Path::new(path);

    if !path.exists() {
        return Err(McpRegError::Manifest(format!(
            "Manifest not found: {}",
            path.display()
        )));
    }

    let content = std::fs::read_to_string(path)?;
    let issues = validate_manifest_content(&content);

    if json_output {
        let result = serde_json::json!({
            "path": path.display().to_string(),
            "valid": issues.is_empty(),
            "issues": issues,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
        return if issues.is_empty() {
            Ok(())
        } else {
            Err(McpRegError::Validation(format!(
                "{} issue(s) found",
                issues.len()
            )))
        };
    }

    println!("Validating {}...\n", path.display());

    if issues.is_empty() {
        println!("✓ Manifest is valid!");
        // Show summary
        let manifest: McpManifest = toml::from_str(&content)?;
        println!("  Name:      {}", manifest.package.name);
        println!("  Version:   {}", manifest.package.version);
        println!("  Command:   {} {}", manifest.server.command, manifest.server.args.join(" "));
        println!("  Transport: {}", manifest.server.transport);
        if !manifest.capabilities.tools.is_empty() {
            println!("  Tools:     {}", manifest.capabilities.tools.join(", "));
        }
        Ok(())
    } else {
        println!("✗ Found {} issue(s):\n", issues.len());
        for (i, issue) in issues.iter().enumerate() {
            println!("  {}. {}", i + 1, issue);
        }
        Err(McpRegError::Validation(format!(
            "{} issue(s) found",
            issues.len()
        )))
    }
}

/// Validate manifest content and return a list of issues (empty = valid).
pub fn validate_manifest_content(content: &str) -> Vec<String> {
    let mut issues = Vec::new();

    let manifest: McpManifest = match toml::from_str(content) {
        Ok(m) => m,
        Err(e) => {
            issues.push(format!("TOML parse error: {e}"));
            return issues;
        }
    };

    // Required fields
    if manifest.package.name.is_empty() {
        issues.push("package.name is required".into());
    } else if manifest.package.name.contains(' ') {
        issues.push("package.name should not contain spaces".into());
    }

    if manifest.package.version.is_empty() {
        issues.push("package.version is required".into());
    } else {
        let parts: Vec<&str> = manifest.package.version.split('.').collect();
        if parts.len() != 3 || parts.iter().any(|p| p.parse::<u32>().is_err()) {
            issues.push("package.version must be in semver format (e.g. 1.0.0)".into());
        }
    }

    if manifest.server.command.is_empty() {
        issues.push("server.command is required".into());
    }

    // Recommended fields (warnings)
    if manifest.package.description.is_empty() {
        issues.push("package.description is empty (recommended)".into());
    }
    if manifest.package.author.is_empty() {
        issues.push("package.author is empty (required for publishing)".into());
    }
    if manifest.package.license.is_empty() {
        issues.push("package.license is empty (recommended)".into());
    }

    // Transport validation
    let valid_transports = ["stdio", "sse", "streamable-http"];
    if !valid_transports.contains(&manifest.server.transport.as_str()) {
        issues.push(format!(
            "server.transport '{}' is not a recognized MCP transport (expected: {})",
            manifest.server.transport,
            valid_transports.join(", ")
        ));
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_manifest() {
        let content = r#"
[package]
name = "test-server"
version = "1.0.0"
description = "A test"
author = "dev"
license = "MIT"

[server]
command = "node"
args = ["index.js"]
transport = "stdio"

[capabilities]
tools = ["tool1"]
"#;
        let issues = validate_manifest_content(content);
        assert!(issues.is_empty(), "Expected no issues, got: {issues:?}");
    }

    #[test]
    fn test_validate_missing_required() {
        let content = r#"
[package]
name = ""
version = "bad"

[server]
command = ""
"#;
        let issues = validate_manifest_content(content);
        assert!(issues.iter().any(|i| i.contains("name is required")));
        assert!(issues.iter().any(|i| i.contains("semver")));
        assert!(issues.iter().any(|i| i.contains("command is required")));
    }

    #[test]
    fn test_validate_bad_transport() {
        let content = r#"
[package]
name = "test"
version = "1.0.0"
description = "test"
author = "dev"
license = "MIT"

[server]
command = "node"
transport = "websocket"
"#;
        let issues = validate_manifest_content(content);
        assert!(issues.iter().any(|i| i.contains("transport")));
    }

    #[test]
    fn test_validate_invalid_toml() {
        let issues = validate_manifest_content("not valid [[toml");
        assert!(!issues.is_empty());
        assert!(issues[0].contains("TOML parse error"));
    }

    #[test]
    fn test_validate_name_with_spaces() {
        let content = r#"
[package]
name = "my bad name"
version = "1.0.0"
description = "test"
author = "dev"
license = "MIT"

[server]
command = "node"
"#;
        let issues = validate_manifest_content(content);
        assert!(issues.iter().any(|i| i.contains("spaces")));
    }
}
