use crate::error::Result;

pub fn run(server: &str, from_version: Option<&str>, json: bool) -> Result<()> {
    let parts: Vec<&str> = server.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(crate::error::McpRegError::Validation(
            "Server must be in owner/name format".into(),
        ));
    }
    let (owner, name) = (parts[0], parts[1]);

    let db_path = crate::config::Config::db_path()
        .unwrap_or_else(|_| std::path::PathBuf::from("registry.db"));

    let db = crate::registry::db::Database::open(db_path.to_str().unwrap_or("registry.db"))?;
    let entry = db
        .get_server(owner, name)?
        .ok_or_else(|| crate::error::McpRegError::NotFound(format!("{owner}/{name}")))?;

    let versions = db.get_version_history(owner, name)?;

    if json {
        let result = serde_json::json!({
            "server": entry.full_name(),
            "current_version": entry.version,
            "from_version": from_version.unwrap_or("(earliest)"),
            "tools": entry.tools,
            "resources": entry.resources,
            "prompts": entry.prompts,
            "tags": entry.tags,
            "transport": entry.transport,
            "versions": versions.iter().map(|(v, d)| serde_json::json!({
                "version": v, "published_at": d
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!("📦 {} v{}", entry.full_name(), entry.version);
    println!();

    if let Some(from) = from_version {
        println!("  Comparing from v{from} → v{}", entry.version);
    }

    println!("  Transport: {}", entry.transport);
    println!("  Tools ({}):", entry.tools.len());
    for t in &entry.tools {
        println!("    • {t}");
    }
    if !entry.resources.is_empty() {
        println!("  Resources ({}):", entry.resources.len());
        for r in &entry.resources {
            println!("    • {r}");
        }
    }
    if !entry.prompts.is_empty() {
        println!("  Prompts ({}):", entry.prompts.len());
        for p in &entry.prompts {
            println!("    • {p}");
        }
    }
    if !entry.tags.is_empty() {
        println!("  Tags: {}", entry.tags.join(", "));
    }

    println!();
    println!("  Version history ({} releases):", versions.len());
    for (v, d) in &versions {
        let marker = if *v == entry.version { " ← current" } else { "" };
        println!("    {v} ({d}){marker}");
    }

    Ok(())
}
