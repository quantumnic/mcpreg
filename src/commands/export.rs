use crate::api::types::InstalledServers;
use crate::config::Config;
use crate::error::Result;
use serde_json::json;

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportFormat {
    Json,
    Toml,
    Env,
}

impl ExportFormat {
    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s.map(|v| v.to_lowercase()).as_deref() {
            Some("toml") => Self::Toml,
            Some("env") => Self::Env,
            _ => Self::Json,
        }
    }
}

pub fn run(output_path: Option<&str>, format: ExportFormat) -> Result<()> {
    let path = Config::installed_servers_path()?;

    if !path.exists() {
        println!("No servers installed. Nothing to export.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let installed: InstalledServers = serde_json::from_str(&content)?;

    if installed.servers.is_empty() {
        println!("No servers installed. Nothing to export.");
        return Ok(());
    }

    let output = match format {
        ExportFormat::Json => export_json(&installed),
        ExportFormat::Toml => export_toml(&installed),
        ExportFormat::Env => export_env(&installed),
    };

    match output_path {
        Some(path) => {
            std::fs::write(path, &output)?;
            println!("✓ Exported {} server(s) to {path} ({:?} format)", installed.servers.len(), format);
        }
        None => {
            println!("{output}");
        }
    }

    Ok(())
}

fn export_json(installed: &InstalledServers) -> String {
    let mut mcp_servers = serde_json::Map::new();
    for server in &installed.servers {
        let mut server_config = serde_json::Map::new();
        server_config.insert("command".into(), json!(server.command));
        server_config.insert("args".into(), json!(server.args));
        mcp_servers.insert(server.name.clone(), serde_json::Value::Object(server_config));
    }
    let config = json!({ "mcpServers": mcp_servers });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

fn export_toml(installed: &InstalledServers) -> String {
    let mut out = String::from("# MCP Server Configuration\n# Exported by mcpreg\n\n");
    for server in &installed.servers {
        out.push_str(&format!("[servers.\"{}\"]\n", server.full_name()));
        out.push_str(&format!("command = \"{}\"\n", server.command));
        if !server.args.is_empty() {
            let args: Vec<String> = server.args.iter().map(|a| format!("\"{a}\"")).collect();
            out.push_str(&format!("args = [{}]\n", args.join(", ")));
        }
        out.push_str(&format!("transport = \"{}\"\n", server.transport));
        out.push_str(&format!("version = \"{}\"\n", server.version));
        out.push('\n');
    }
    out
}

fn export_env(installed: &InstalledServers) -> String {
    let mut out = String::from("# MCP Server Environment Variables\n# Exported by mcpreg\n\n");
    for (i, server) in installed.servers.iter().enumerate() {
        let prefix = format!("MCP_SERVER_{}", i);
        out.push_str(&format!("# {}\n", server.full_name()));
        out.push_str(&format!("{prefix}_NAME=\"{}\"\n", server.full_name()));
        out.push_str(&format!("{prefix}_COMMAND=\"{}\"\n", server.command));
        if !server.args.is_empty() {
            out.push_str(&format!("{prefix}_ARGS=\"{}\"\n", server.args.join(" ")));
        }
        out.push_str(&format!("{prefix}_TRANSPORT=\"{}\"\n", server.transport));
        out.push_str(&format!("{prefix}_VERSION=\"{}\"\n", server.version));
        out.push('\n');
    }
    out.push_str(&format!("MCP_SERVER_COUNT=\"{}\"\n", installed.servers.len()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{InstalledServer, InstalledServers};

    fn sample_installed() -> InstalledServers {
        InstalledServers {
            servers: vec![
                InstalledServer {
                    owner: "org".into(),
                    name: "filesystem".into(),
                    version: "1.0.0".into(),
                    command: "npx".into(),
                    args: vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into()],
                    transport: "stdio".into(),
                    installed_at: "2024-01-01T00:00:00Z".into(),
                },
                InstalledServer {
                    owner: "org".into(),
                    name: "sqlite".into(),
                    version: "2.0.0".into(),
                    command: "uvx".into(),
                    args: vec!["mcp-server-sqlite".into()],
                    transport: "stdio".into(),
                    installed_at: "2024-01-02T00:00:00Z".into(),
                },
            ],
        }
    }

    #[test]
    fn test_export_format_from_str() {
        assert_eq!(ExportFormat::from_str_opt(None), ExportFormat::Json);
        assert_eq!(ExportFormat::from_str_opt(Some("json")), ExportFormat::Json);
        assert_eq!(ExportFormat::from_str_opt(Some("toml")), ExportFormat::Toml);
        assert_eq!(ExportFormat::from_str_opt(Some("TOML")), ExportFormat::Toml);
        assert_eq!(ExportFormat::from_str_opt(Some("env")), ExportFormat::Env);
        assert_eq!(ExportFormat::from_str_opt(Some("ENV")), ExportFormat::Env);
        assert_eq!(ExportFormat::from_str_opt(Some("unknown")), ExportFormat::Json);
    }

    #[test]
    fn test_export_json_format() {
        let installed = sample_installed();
        let output = export_json(&installed);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["mcpServers"]["filesystem"].is_object());
        assert!(parsed["mcpServers"]["sqlite"].is_object());
        assert_eq!(parsed["mcpServers"]["filesystem"]["command"], "npx");
    }

    #[test]
    fn test_export_toml_format() {
        let installed = sample_installed();
        let output = export_toml(&installed);
        assert!(output.contains("[servers.\"org/filesystem\"]"));
        assert!(output.contains("command = \"npx\""));
        assert!(output.contains("transport = \"stdio\""));
        assert!(output.contains("[servers.\"org/sqlite\"]"));
    }

    #[test]
    fn test_export_env_format() {
        let installed = sample_installed();
        let output = export_env(&installed);
        assert!(output.contains("MCP_SERVER_0_NAME=\"org/filesystem\""));
        assert!(output.contains("MCP_SERVER_0_COMMAND=\"npx\""));
        assert!(output.contains("MCP_SERVER_1_NAME=\"org/sqlite\""));
        assert!(output.contains("MCP_SERVER_COUNT=\"2\""));
    }

    #[test]
    fn test_export_json_empty() {
        let installed = InstalledServers { servers: vec![] };
        let output = export_json(&installed);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["mcpServers"].as_object().unwrap().is_empty());
    }

    #[test]
    fn test_export_toml_empty() {
        let installed = InstalledServers { servers: vec![] };
        let output = export_toml(&installed);
        assert!(output.contains("# MCP Server Configuration"));
        assert!(!output.contains("[servers."));
    }

    #[test]
    fn test_export_env_empty() {
        let installed = InstalledServers { servers: vec![] };
        let output = export_env(&installed);
        assert!(output.contains("MCP_SERVER_COUNT=\"0\""));
    }

    #[test]
    fn test_export_no_servers() {
        // Should not panic when no installed.json exists
        let _ = run(None, ExportFormat::Json);
    }

    #[test]
    fn test_export_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("export.json");
        let _ = run(Some(out.to_str().unwrap()), ExportFormat::Json);
    }
}
