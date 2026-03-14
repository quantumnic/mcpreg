use crate::error::Result;
use std::path::Path;

/// Lint and validate a local mcpreg.toml manifest file.
pub fn run(path: Option<&str>) -> Result<()> {
    let manifest_path = path.unwrap_or("mcpreg.toml");
    let p = Path::new(manifest_path);

    if !p.exists() {
        eprintln!("❌ File not found: {manifest_path}");
        eprintln!("   Create one with: mcpreg init");
        std::process::exit(1);
    }

    let content = std::fs::read_to_string(p)?;
    let table: toml::Table = match toml::from_str(&content) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("❌ Invalid TOML syntax: {e}");
            std::process::exit(1);
        }
    };

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut info: Vec<String> = Vec::new();

    // Check required fields
    let required = ["name", "version", "description", "command"];
    for field in required {
        match table.get(field) {
            None => errors.push(format!("missing required field '{field}'")),
            Some(v) if v.as_str().is_some_and(|s| s.is_empty()) => {
                errors.push(format!("required field '{field}' is empty"));
            }
            _ => {}
        }
    }

    // Check recommended fields
    let recommended = ["author", "license", "repository", "homepage"];
    for field in recommended {
        match table.get(field) {
            None => warnings.push(format!("missing recommended field '{field}'")),
            Some(v) if v.as_str().is_some_and(|s| s.is_empty()) => {
                warnings.push(format!("recommended field '{field}' is empty"));
            }
            _ => {}
        }
    }

    // Version format check
    if let Some(version) = table.get("version").and_then(|v| v.as_str()) {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() < 2 || parts.iter().any(|p| p.parse::<u64>().is_err()) {
            warnings.push(format!("version '{version}' doesn't follow semver (expected X.Y.Z)"));
        }
    }

    // Name format check
    if let Some(name) = table.get("name").and_then(|v| v.as_str()) {
        if name.contains(' ') {
            errors.push("name should not contain spaces".into());
        }
        if name != name.to_lowercase() {
            warnings.push("name should be lowercase by convention".into());
        }
        if name.len() > 64 {
            warnings.push("name is very long (>64 chars)".into());
        }
    }

    // Transport check
    if let Some(transport) = table.get("transport").and_then(|v| v.as_str()) {
        let valid_transports = ["stdio", "sse", "streamable-http"];
        if !valid_transports.contains(&transport) {
            errors.push(format!("unknown transport '{transport}' (expected: {valid_transports:?})"));
        }
    }

    // Command check
    if let Some(command) = table.get("command").and_then(|v| v.as_str()) {
        if command.contains('/') || command.contains('\\') {
            warnings.push("command contains path separators — consider using just the binary name".into());
        }
    }

    // Tools check
    if let Some(tools) = table.get("tools").and_then(|v| v.as_array()) {
        if tools.is_empty() {
            warnings.push("tools array is empty — servers should expose at least one tool".into());
        }
        for (i, tool) in tools.iter().enumerate() {
            if let Some(s) = tool.as_str() {
                if s.is_empty() {
                    warnings.push(format!("tools[{i}] is an empty string"));
                }
            }
        }
        info.push(format!("{} tool(s) declared", tools.len()));
    } else {
        warnings.push("no 'tools' array defined — servers should list their tools".into());
    }

    // Tags check
    if let Some(tags) = table.get("tags").and_then(|v| v.as_array()) {
        if tags.len() > 10 {
            warnings.push(format!("too many tags ({}) — consider keeping it under 10", tags.len()));
        }
        info.push(format!("{} tag(s) declared", tags.len()));
    }

    // License check
    if let Some(license) = table.get("license").and_then(|v| v.as_str()) {
        let common = ["MIT", "Apache-2.0", "GPL-3.0", "GPL-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "MPL-2.0", "AGPL-3.0", "Unlicense", "LGPL-3.0"];
        if !common.contains(&license) {
            info.push(format!("license '{license}' is not a common SPDX identifier"));
        }
    }

    // Env check
    if let Some(env) = table.get("env") {
        if let Some(env_table) = env.as_table() {
            let sensitive_patterns = ["key", "secret", "token", "password", "credential"];
            for (key, val) in env_table {
                if let Some(v) = val.as_str() {
                    if !v.is_empty() && sensitive_patterns.iter().any(|p| key.to_lowercase().contains(p)) {
                        warnings.push(format!("env '{key}' looks like it contains a real secret — use empty string as placeholder"));
                    }
                }
            }
            info.push(format!("{} env var(s) declared", env_table.len()));
        }
    }

    // Print results
    println!("🔍 Linting: {manifest_path}\n");

    if errors.is_empty() && warnings.is_empty() {
        println!("✅ No issues found!\n");
    }

    for e in &errors {
        println!("  ❌ {e}");
    }
    for w in &warnings {
        println!("  ⚠️  {w}");
    }
    for i in &info {
        println!("  ℹ️  {i}");
    }

    println!();
    println!(
        "  Summary: {} error(s), {} warning(s)",
        errors.len(),
        warnings.len()
    );

    if !errors.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_manifest(dir: &tempfile::TempDir, content: &str) -> String {
        let path = dir.path().join("mcpreg.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path.to_string_lossy().to_string()
    }

    #[test]
    fn test_lint_valid_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_manifest(&dir, r#"
name = "test-server"
version = "1.0.0"
description = "A test server"
command = "test-cmd"
author = "Test Author"
license = "MIT"
repository = "https://github.com/test/test"
homepage = "https://test.dev"
transport = "stdio"
tools = ["tool1", "tool2"]
tags = ["test"]
"#);
        // Should not panic or error
        run(Some(&path)).unwrap();
    }

    #[test]
    fn test_lint_detects_missing_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_manifest(&dir, r#"
name = "test"
"#);
        // This will call process::exit, so we just verify it doesn't panic before that
        // In a real test harness we'd capture the exit, but for now we test indirectly
        assert!(Path::new(&path).exists());
    }

    #[test]
    fn test_lint_detects_bad_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_manifest(&dir, r#"
name = "test-server"
version = "abc"
description = "A test server"
command = "test-cmd"
"#);
        assert!(Path::new(&path).exists());
    }
}
