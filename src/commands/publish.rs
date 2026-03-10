use crate::api::client::RegistryClient;
use crate::api::types::{McpManifest, ServerEntry};
use crate::config::Config;
use crate::error::{McpRegError, Result};
use std::path::Path;

pub async fn run(manifest_path: Option<&str>) -> Result<()> {
    let path = manifest_path.unwrap_or("mcpreg.toml");
    let path = Path::new(path);

    if !path.exists() {
        return Err(McpRegError::Manifest(format!(
            "Manifest file not found: {}. Create an mcpreg.toml in your project root.",
            path.display()
        )));
    }

    let content = std::fs::read_to_string(path)?;
    let manifest: McpManifest = toml::from_str(&content).map_err(|e| {
        McpRegError::Manifest(format!("Invalid mcpreg.toml: {e}"))
    })?;

    validate_manifest(&manifest)?;

    let config = Config::load()?;
    if config.api_key.is_none() {
        return Err(McpRegError::Auth(
            "API key required for publishing. Set api_key in ~/.mcpreg/config.toml".into(),
        ));
    }

    let owner = &manifest.package.author;
    if owner.is_empty() {
        return Err(McpRegError::Manifest(
            "Package author is required for publishing".into(),
        ));
    }

    let entry = ServerEntry::from_manifest(owner, &manifest);
    let client = RegistryClient::new(&config);

    println!("Publishing {}/{} v{}...", entry.owner, entry.name, entry.version);
    let response = client.publish(&entry).await?;

    if response.success {
        println!("✓ {}", response.message);
    } else {
        println!("✗ Failed: {}", response.message);
    }

    Ok(())
}

fn validate_manifest(manifest: &McpManifest) -> Result<()> {
    if manifest.package.name.is_empty() {
        return Err(McpRegError::Manifest("Package name is required".into()));
    }
    if manifest.package.version.is_empty() {
        return Err(McpRegError::Manifest("Package version is required".into()));
    }
    if manifest.server.command.is_empty() {
        return Err(McpRegError::Manifest("Server command is required".into()));
    }
    // Validate version format (basic semver check)
    let parts: Vec<&str> = manifest.package.version.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.parse::<u32>().is_err()) {
        return Err(McpRegError::Manifest(
            "Version must be in semver format (e.g. 1.0.0)".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{Capabilities, PackageInfo, ServerInfo};

    fn valid_manifest() -> McpManifest {
        McpManifest {
            package: PackageInfo {
                name: "test".into(),
                version: "1.0.0".into(),
                description: "Test".into(),
                author: "user".into(),
                license: "MIT".into(),
                repository: String::new(),
            },
            server: ServerInfo {
                command: "node".into(),
                args: vec!["index.js".into()],
                transport: "stdio".into(),
                env: Default::default(),
            },
            capabilities: Capabilities::default(),
        }
    }

    #[test]
    fn test_validate_valid_manifest() {
        assert!(validate_manifest(&valid_manifest()).is_ok());
    }

    #[test]
    fn test_validate_missing_name() {
        let mut m = valid_manifest();
        m.package.name = String::new();
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn test_validate_bad_version() {
        let mut m = valid_manifest();
        m.package.version = "not-semver".into();
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn test_validate_missing_command() {
        let mut m = valid_manifest();
        m.server.command = String::new();
        assert!(validate_manifest(&m).is_err());
    }
}
