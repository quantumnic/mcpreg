use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

/// Autocomplete server names by prefix — works offline against the local DB.
pub async fn run(prefix: &str, limit: usize, json: bool) -> Result<()> {
    let db_path = Config::db_path()?;
    let db = Database::open(db_path.to_str().unwrap_or("registry.db"))?;

    let suggestions = db.suggest(prefix, limit)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "prefix": prefix,
            "suggestions": suggestions,
            "total": suggestions.len(),
        }))?);
    } else if suggestions.is_empty() {
        println!("No matches for '{prefix}'");
    } else {
        for s in &suggestions {
            println!("{s}");
        }
    }

    Ok(())
}
