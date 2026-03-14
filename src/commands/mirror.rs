use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

pub fn run(output_dir: &str, json_output: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());
    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    let all = db.list_all()?;

    if all.is_empty() {
        if !json_output {
            println!("Registry is empty — nothing to mirror.");
        }
        return Ok(());
    }

    let output = std::path::Path::new(output_dir);
    std::fs::create_dir_all(output)?;

    let mut owner_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for entry in &all {
        let owner_dir = output.join(&entry.owner);
        std::fs::create_dir_all(&owner_dir)?;

        let file = owner_dir.join(format!("{}.json", entry.name));
        let content = serde_json::to_string_pretty(&entry)?;
        std::fs::write(&file, content)?;
        *owner_counts.entry(entry.owner.clone()).or_default() += 1;
    }

    // Write index.json with summary
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let index = serde_json::json!({
        "total_servers": all.len(),
        "owners": owner_counts.len(),
        "mirrored_at": now_secs,
        "servers": all.iter().map(|s| s.full_name()).collect::<Vec<_>>(),
    });
    let index_path = output.join("index.json");
    std::fs::write(&index_path, serde_json::to_string_pretty(&index)?)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "mirrored": all.len(),
            "owners": owner_counts.len(),
            "output": output_dir,
        }))?);
    } else {
        println!(
            "✅ Mirrored {} servers from {} owners to {}",
            all.len(),
            owner_counts.len(),
            output_dir
        );
        println!("   Index: {}", index_path.display());
    }

    Ok(())
}
