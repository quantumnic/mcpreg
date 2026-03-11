use crate::error::{McpRegError, Result};
use std::path::Path;

pub fn run(path: Option<&str>) -> Result<()> {
    let manifest_path = match path {
        Some(p) => std::path::PathBuf::from(p).join("mcpreg.toml"),
        None => std::path::PathBuf::from("mcpreg.toml"),
    };

    if manifest_path.exists() {
        return Err(McpRegError::Manifest(format!(
            "{} already exists",
            manifest_path.display()
        )));
    }

    // Try to infer project name from directory
    let project_name = infer_project_name(manifest_path.parent());

    let template = format!(
        r#"[package]
name = "{project_name}"
version = "0.1.0"
description = ""
author = ""
license = "MIT"
repository = ""

[server]
command = "node"
args = ["dist/index.js"]
transport = "stdio"

# [server.env]
# API_KEY = "placeholder"

[capabilities]
tools = []
resources = []
prompts = []
"#
    );

    if let Some(parent) = manifest_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    std::fs::write(&manifest_path, template)?;
    println!("✓ Created {}", manifest_path.display());
    println!("  Edit it to describe your MCP server, then run 'mcpreg publish'");
    Ok(())
}

fn infer_project_name(dir: Option<&Path>) -> String {
    dir.and_then(|d| {
        let abs = if d.as_os_str().is_empty() {
            std::env::current_dir().ok()?
        } else {
            d.to_path_buf()
        };
        abs.file_name()
            .map(|n| n.to_string_lossy().to_string())
    })
    .unwrap_or_else(|| "my-mcp-server".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_creates_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        run(Some(path)).unwrap();
        let manifest = dir.path().join("mcpreg.toml");
        assert!(manifest.exists());
        let content = std::fs::read_to_string(&manifest).unwrap();
        assert!(content.contains("[package]"));
        assert!(content.contains("[server]"));
        assert!(content.contains("[capabilities]"));
    }

    #[test]
    fn test_init_refuses_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("mcpreg.toml");
        std::fs::write(&manifest, "existing").unwrap();
        let result = run(Some(dir.path().to_str().unwrap()));
        assert!(result.is_err());
    }
}
