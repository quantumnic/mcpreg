use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

/// Import servers from a JSON file (exported by `mcpreg export` or snapshot).
pub fn run(file: &str, dry_run: bool, json_output: bool) -> Result<()> {
    let content = std::fs::read_to_string(file)
        .map_err(crate::error::McpRegError::Io)?;

    // Try parsing as a full export ({"servers": [...]}) or bare array
    let servers: Vec<crate::api::types::ServerEntry> = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
        if let Some(arr) = v.get("servers").and_then(|s| s.as_array()) {
            arr.iter()
                .filter_map(|entry| serde_json::from_value(entry.clone()).ok())
                .collect()
        } else if v.is_array() {
            serde_json::from_value(v).unwrap_or_default()
        } else {
            eprintln!("Error: Expected JSON with 'servers' array or a bare array");
            return Ok(());
        }
    } else {
        eprintln!("Error: Invalid JSON in {file}");
        return Ok(());
    };

    if json_output {
        if dry_run {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "dry_run": true,
                "would_import": servers.len(),
                "servers": servers.iter().map(|s| s.full_name()).collect::<Vec<_>>(),
            }))?);
        } else {
            let db_path = Config::db_path()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "registry.db".to_string());
            let db = Database::open(&db_path)?;
            let mut imported = 0;
            let mut errors = Vec::new();
            for entry in &servers {
                match db.upsert_server(entry) {
                    Ok(_) => imported += 1,
                    Err(e) => errors.push(format!("{}: {e}", entry.full_name())),
                }
            }
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "imported": imported,
                "errors": errors,
                "total_in_file": servers.len(),
            }))?);
        }
        return Ok(());
    }

    if dry_run {
        println!("Would import {} server(s) from '{file}':", servers.len());
        for s in &servers {
            println!("  {} v{}", s.full_name(), s.version);
        }
        return Ok(());
    }

    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());
    let db = Database::open(&db_path)?;

    let mut imported = 0;
    let mut errors = 0;
    for entry in &servers {
        match db.upsert_server(entry) {
            Ok(_) => {
                imported += 1;
                println!("  ✓ {} v{}", entry.full_name(), entry.version);
            }
            Err(e) => {
                errors += 1;
                eprintln!("  ✗ {}: {e}", entry.full_name());
            }
        }
    }

    println!("\nImported {imported} server(s) ({errors} error(s)) from '{file}'");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::api::types::ServerEntry;

    fn make_server(owner: &str, name: &str) -> ServerEntry {
        ServerEntry {
            id: None,
            owner: owner.into(),
            name: name.into(),
            version: "1.0.0".into(),
            description: format!("{name} server"),
            author: owner.into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec!["tool1".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_parse_export_format() {
        let servers = vec![make_server("a", "b"), make_server("c", "d")];
        let json = serde_json::json!({"servers": servers});
        let s = serde_json::to_string(&json).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        let arr = v.get("servers").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_parse_bare_array_format() {
        let servers = vec![make_server("a", "b")];
        let s = serde_json::to_string(&servers).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v.is_array());
        let entries: Vec<ServerEntry> = serde_json::from_value(v).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_import_into_db() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let entry = make_server("test", "import");
        db.upsert_server(&entry).unwrap();
        let fetched = db.get_server("test", "import").unwrap().unwrap();
        assert_eq!(fetched.full_name(), "test/import");
        assert_eq!(fetched.version, "1.0.0");
    }

    #[test]
    fn test_import_updates_existing() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let mut entry = make_server("test", "update");
        db.upsert_server(&entry).unwrap();

        entry.version = "2.0.0".into();
        entry.description = "Updated".into();
        db.upsert_server(&entry).unwrap();

        let fetched = db.get_server("test", "update").unwrap().unwrap();
        assert_eq!(fetched.version, "2.0.0");
        assert_eq!(fetched.description, "Updated");
    }
}
