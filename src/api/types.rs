use serde::{Deserialize, Serialize};

/// The mcpreg.toml manifest format
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpManifest {
    pub package: PackageInfo,
    pub server: ServerInfo,
    #[serde(default)]
    pub capabilities: Capabilities,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub repository: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerInfo {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_transport")]
    pub transport: String,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

fn default_transport() -> String {
    "stdio".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Capabilities {
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub resources: Vec<String>,
    #[serde(default)]
    pub prompts: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Registry server entry (stored in DB / returned by API)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerEntry {
    #[serde(default)]
    pub id: Option<i64>,
    pub owner: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub repository: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_transport")]
    pub transport: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub resources: Vec<String>,
    #[serde(default)]
    pub prompts: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Environment variable hints for server configuration (e.g. API keys, paths)
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// Optional homepage URL (separate from repository)
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub deprecated_by: Option<String>,
    #[serde(default)]
    pub downloads: i64,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl ServerEntry {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    pub fn from_manifest(owner: &str, manifest: &McpManifest) -> Self {
        Self {
            id: None,
            owner: owner.to_string(),
            name: manifest.package.name.clone(),
            version: manifest.package.version.clone(),
            description: manifest.package.description.clone(),
            author: manifest.package.author.clone(),
            license: manifest.package.license.clone(),
            repository: manifest.package.repository.clone(),
            command: manifest.server.command.clone(),
            args: manifest.server.args.clone(),
            transport: manifest.server.transport.clone(),
            tools: manifest.capabilities.tools.clone(),
            resources: manifest.capabilities.resources.clone(),
            prompts: manifest.capabilities.prompts.clone(),
            tags: manifest.capabilities.tags.clone(),
            env: manifest.server.env.clone(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            created_at: None,
            updated_at: None,
        }
    }
}

/// API response wrappers
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub servers: Vec<ServerEntry>,
    pub total: usize,
    /// Fuzzy "did you mean?" suggestions when search returns 0 results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestions: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PublishResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedResponse {
    pub servers: Vec<ServerEntry>,
    pub page: usize,
    pub per_page: usize,
    pub total: usize,
}

/// Installed server tracking
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstalledServer {
    pub owner: String,
    pub name: String,
    pub version: String,
    pub command: String,
    pub args: Vec<String>,
    pub transport: String,
    pub installed_at: String,
}

impl InstalledServer {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct InstalledServers {
    pub servers: Vec<InstalledServer>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manifest() {
        let toml_str = r#"
[package]
name = "test-server"
version = "1.0.0"
description = "A test server"
author = "testuser"
license = "MIT"
repository = "https://github.com/test/repo"

[server]
command = "node"
args = ["dist/index.js"]
transport = "stdio"

[capabilities]
tools = ["read_file", "write_file"]
resources = ["file://"]
"#;
        let manifest: McpManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.package.name, "test-server");
        assert_eq!(manifest.server.command, "node");
        assert_eq!(manifest.server.args, vec!["dist/index.js"]);
        assert_eq!(manifest.capabilities.tools.len(), 2);
    }

    #[test]
    fn test_parse_minimal_manifest() {
        let toml_str = r#"
[package]
name = "minimal"
version = "0.1.0"

[server]
command = "python"
"#;
        let manifest: McpManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.package.name, "minimal");
        assert_eq!(manifest.server.transport, "stdio");
        assert!(manifest.capabilities.tools.is_empty());
    }

    #[test]
    fn test_server_entry_from_manifest() {
        let toml_str = r#"
[package]
name = "my-server"
version = "2.0.0"
description = "Cool server"
author = "dev"
license = "Apache-2.0"
repository = "https://github.com/dev/server"

[server]
command = "npx"
args = ["-y", "my-server"]
transport = "stdio"

[capabilities]
tools = ["tool1"]
"#;
        let manifest: McpManifest = toml::from_str(toml_str).unwrap();
        let entry = ServerEntry::from_manifest("dev", &manifest);
        assert_eq!(entry.full_name(), "dev/my-server");
        assert_eq!(entry.version, "2.0.0");
        assert_eq!(entry.downloads, 0);
    }

    #[test]
    fn test_server_entry_full_name() {
        let entry = ServerEntry {
            id: Some(1),
            owner: "alice".into(),
            name: "filesystem".into(),
            version: "1.0.0".into(),
            description: String::new(),
            author: String::new(),
            license: String::new(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 42,
            created_at: None,
            updated_at: None,
        };
        assert_eq!(entry.full_name(), "alice/filesystem");
    }

    #[test]
    fn test_installed_server_full_name() {
        let s = InstalledServer {
            owner: "bob".into(),
            name: "sqlite".into(),
            version: "1.2.0".into(),
            command: "uvx".into(),
            args: vec!["mcp-server-sqlite".into()],
            transport: "stdio".into(),
            installed_at: "2024-01-01T00:00:00Z".into(),
        };
        assert_eq!(s.full_name(), "bob/sqlite");
    }

    #[test]
    fn test_manifest_with_env() {
        let toml_str = r#"
[package]
name = "env-server"
version = "1.0.0"

[server]
command = "node"
args = ["index.js"]

[server.env]
API_KEY = "placeholder"
DEBUG = "true"
"#;
        let manifest: McpManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.server.env.len(), 2);
        assert_eq!(manifest.server.env.get("DEBUG").unwrap(), "true");
    }
}

#[cfg(test)]
mod prompts_tests {
    use super::*;

    #[test]
    fn test_manifest_with_prompts() {
        let toml_str = r#"
[package]
name = "prompt-server"
version = "1.0.0"
description = "Server with prompts"
author = "dev"

[server]
command = "node"
args = ["index.js"]

[capabilities]
tools = ["tool1"]
resources = []
prompts = ["summarize", "code-review", "translate"]
"#;
        let manifest: McpManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.capabilities.prompts.len(), 3);
        assert_eq!(manifest.capabilities.prompts[0], "summarize");
    }

    #[test]
    fn test_server_entry_from_manifest_includes_prompts() {
        let toml_str = r#"
[package]
name = "prompt-test"
version = "1.0.0"
author = "dev"

[server]
command = "node"

[capabilities]
prompts = ["analyze", "summarize"]
"#;
        let manifest: McpManifest = toml::from_str(toml_str).unwrap();
        let entry = ServerEntry::from_manifest("dev", &manifest);
        assert_eq!(entry.prompts, vec!["analyze", "summarize"]);
    }

    #[test]
    fn test_server_entry_json_roundtrip_with_prompts() {
        let entry = ServerEntry {
            id: Some(1),
            owner: "test".into(),
            name: "roundtrip".into(),
            version: "1.0.0".into(),
            description: "Test".into(),
            author: "test".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec!["index.js".into()],
            transport: "stdio".into(),
            tools: vec!["tool1".into()],
            resources: vec![],
            prompts: vec!["prompt1".into(), "prompt2".into()],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 42,
            created_at: Some("2024-01-01".into()),
            updated_at: Some("2024-01-02".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: ServerEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.prompts, vec!["prompt1", "prompt2"]);
    }

    #[test]
    fn test_server_entry_json_missing_prompts_defaults_empty() {
        // Old API responses without prompts field should deserialize fine
        let json = r#"{
            "id": 1,
            "owner": "test",
            "name": "old-format",
            "version": "1.0.0",
            "description": "",
            "author": "",
            "license": "",
            "repository": "",
            "command": "node",
            "args": [],
            "transport": "stdio",
            "tools": [],
            "resources": [],
            "downloads": 0
        }"#;
        let entry: ServerEntry = serde_json::from_str(json).unwrap();
        assert!(entry.prompts.is_empty(), "Missing prompts should default to empty");
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;

    #[test]
    fn test_server_entry_full_name() {
        let entry = ServerEntry {
            id: None,
            owner: "alice".into(),
            name: "tool".into(),
            version: "1.0.0".into(),
            description: String::new(),
            author: String::new(),
            license: String::new(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            created_at: None,
            updated_at: None,
        };
        assert_eq!(entry.full_name(), "alice/tool");
    }

    #[test]
    fn test_installed_server_full_name() {
        let server = InstalledServer {
            owner: "bob".into(),
            name: "service".into(),
            version: "2.0.0".into(),
            command: "npx".into(),
            args: vec![],
            transport: "stdio".into(),
            installed_at: "2024-01-01".into(),
        };
        assert_eq!(server.full_name(), "bob/service");
    }

    #[test]
    fn test_server_entry_from_manifest() {
        let manifest = McpManifest {
            package: PackageInfo {
                name: "my-tool".into(),
                version: "3.0.0".into(),
                description: "A cool tool".into(),
                author: "dev".into(),
                license: "MIT".into(),
                repository: "https://github.com/dev/my-tool".into(),
            },
            server: ServerInfo {
                command: "node".into(),
                args: vec!["dist/index.js".into()],
                transport: "stdio".into(),
                env: Default::default(),
            },
            capabilities: Capabilities {
                tools: vec!["do_stuff".into()],
                resources: vec![],
                prompts: vec!["helper".into()],
                tags: vec![],
            },
        };
        let entry = ServerEntry::from_manifest("dev", &manifest);
        assert_eq!(entry.owner, "dev");
        assert_eq!(entry.name, "my-tool");
        assert_eq!(entry.version, "3.0.0");
        assert_eq!(entry.tools, vec!["do_stuff"]);
        assert_eq!(entry.prompts, vec!["helper"]);
    }

    #[test]
    fn test_installed_servers_default_empty() {
        let installed = InstalledServers::default();
        assert!(installed.servers.is_empty());
    }

    #[test]
    fn test_search_response_serde_roundtrip() {
        let resp = SearchResponse {
            total: 1,
            suggestions: None,
            servers: vec![ServerEntry {
                id: Some(1),
                owner: "a".into(),
                name: "b".into(),
                version: "1.0.0".into(),
                description: "test".into(),
                author: "x".into(),
                license: "MIT".into(),
                repository: String::new(),
                command: "node".into(),
                args: vec![],
                transport: "stdio".into(),
                tools: vec!["t1".into()],
                resources: vec![],
                prompts: vec![],
                tags: vec![],
                env: Default::default(),
                homepage: String::new(),
                deprecated: false,
                deprecated_by: None,
                downloads: 42,
                created_at: None,
                updated_at: None,
            }],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: SearchResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.total, 1);
        assert_eq!(back.servers[0].downloads, 42);
    }
}
