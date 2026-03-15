use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

/// Generate an MCP configuration file from installed or all seeded servers.
///
/// Output formats: JSON (Claude Desktop style) or TOML (mcpreg native).
pub fn run(format: &str, filter_owner: Option<&str>, filter_tag: Option<&str>, installed_only: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());
    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    let servers = if installed_only {
        let installed_path = Config::installed_servers_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "installed.json".to_string());
        let installed: crate::api::types::InstalledServers = if let Ok(data) = std::fs::read_to_string(&installed_path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Default::default()
        };
        // Match installed against DB for full metadata
        let mut matched = Vec::new();
        for inst in &installed.servers {
            if let Ok(Some(entry)) = db.get_server(&inst.owner, &inst.name) {
                matched.push(entry);
            }
        }
        matched
    } else {
        db.list_all()?
    };

    let filtered: Vec<_> = servers.into_iter().filter(|s| {
        if let Some(owner) = filter_owner {
            if s.owner != owner { return false; }
        }
        if let Some(tag) = filter_tag {
            if !s.tags.iter().any(|t| t == tag) { return false; }
        }
        !s.deprecated
    }).collect();

    match format {
        "json" => print_json_config(&filtered),
        "toml" => print_toml_config(&filtered),
        "env" => print_env_config(&filtered),
        _ => {
            eprintln!("Unknown format '{}'. Use: json, toml, env", format);
        }
    }

    Ok(())
}

fn print_json_config(servers: &[crate::api::types::ServerEntry]) {
    println!("{{");
    println!("  \"mcpServers\": {{");
    for (i, s) in servers.iter().enumerate() {
        let comma = if i + 1 < servers.len() { "," } else { "" };
        let args_json: Vec<String> = s.args.iter().map(|a| format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\""))).collect();
        let args_str = args_json.join(", ");

        println!("    \"{}\": {{", s.name);
        println!("      \"command\": \"{}\",", s.command);
        println!("      \"args\": [{}]", args_str);

        if !s.env.is_empty() {
            println!("      ,\"env\": {{");
            let env_entries: Vec<_> = s.env.iter().collect();
            for (j, (k, v)) in env_entries.iter().enumerate() {
                let env_comma = if j + 1 < env_entries.len() { "," } else { "" };
                println!("        \"{}\": \"{}\"{}",
                    k.replace('"', "\\\""),
                    v.replace('"', "\\\""),
                    env_comma
                );
            }
            println!("      }}");
        }

        println!("    }}{comma}");
    }
    println!("  }}");
    println!("}}");
}

fn print_toml_config(servers: &[crate::api::types::ServerEntry]) {
    println!("# Generated MCP configuration");
    println!("# {} server(s)\n", servers.len());
    for s in servers {
        println!("[servers.\"{}/{}\"]", s.owner, s.name);
        println!("command = \"{}\"", s.command);
        if !s.args.is_empty() {
            let args: Vec<String> = s.args.iter().map(|a| format!("\"{}\"", a)).collect();
            println!("args = [{}]", args.join(", "));
        }
        println!("transport = \"{}\"", s.transport);
        if !s.env.is_empty() {
            for (k, v) in &s.env {
                println!("env.{} = \"{}\"", k, v);
            }
        }
        println!();
    }
}

fn print_env_config(servers: &[crate::api::types::ServerEntry]) {
    println!("# Environment variables needed by MCP servers");
    println!("# {} server(s)\n", servers.len());
    for s in servers {
        if !s.env.is_empty() {
            println!("# {}/{}", s.owner, s.name);
            for (k, v) in &s.env {
                println!("{}={}", k, v);
            }
            println!();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_server() -> crate::api::types::ServerEntry {
        crate::api::types::ServerEntry {
            id: Some(1),
            owner: "test".into(),
            name: "my-server".into(),
            version: "1.0.0".into(),
            description: "A test server".into(),
            author: "dev".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec!["dist/index.js".into()],
            transport: "stdio".into(),
            tools: vec!["read".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec!["test".into()],
            env: std::collections::HashMap::from([
                ("API_KEY".into(), "changeme".into()),
            ]),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 100,
            stars: 5,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_print_json_config_no_panic() {
        let servers = vec![test_server()];
        // Just verify it doesn't panic
        print_json_config(&servers);
    }

    #[test]
    fn test_print_toml_config_no_panic() {
        let servers = vec![test_server()];
        print_toml_config(&servers);
    }

    #[test]
    fn test_print_env_config_no_panic() {
        let servers = vec![test_server()];
        print_env_config(&servers);
    }

    #[test]
    fn test_print_json_empty() {
        print_json_config(&[]);
    }

    #[test]
    fn test_print_toml_empty() {
        print_toml_config(&[]);
    }

    #[test]
    fn test_print_env_empty() {
        print_env_config(&[]);
    }

    #[test]
    fn test_print_json_multiple_servers() {
        let mut s1 = test_server();
        s1.name = "server-a".into();
        let mut s2 = test_server();
        s2.name = "server-b".into();
        s2.env.clear();
        print_json_config(&[s1, s2]);
    }

    #[test]
    fn test_print_env_skips_no_env() {
        let mut s = test_server();
        s.env.clear();
        print_env_config(&[s]);
    }
}
