use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

/// Show a timeline view of recent registry activity (new servers, updates, milestones).
pub fn run(limit: usize, json_output: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());
    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    // Get recent versions (acts as our changelog)
    let versions = db.recent_versions(limit)?;

    if json_output {
        let entries: Vec<serde_json::Value> = versions
            .iter()
            .map(|(owner, name, version, date)| {
                serde_json::json!({
                    "owner": owner,
                    "name": name,
                    "version": version,
                    "published_at": date,
                    "server": format!("{owner}/{name}"),
                })
            })
            .collect();
        let output = serde_json::json!({
            "timeline": entries,
            "count": entries.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    if versions.is_empty() {
        println!("No recent activity in the registry.");
        return Ok(());
    }

    println!("📅 Registry Timeline (last {} events):\n", versions.len());

    let mut current_date = String::new();
    for (owner, name, version, date) in &versions {
        // Group by date (show date headers)
        let day = date.split('T').next().unwrap_or(date).split(' ').next().unwrap_or(date);
        if day != current_date {
            if !current_date.is_empty() {
                println!();
            }
            println!(
                "  ─ {d}{day}{r}",
                d = crate::color::dim(),
                r = crate::color::reset(),
            );
            current_date = day.to_string();
        }

        let time_part = date
            .split('T')
            .nth(1)
            .or_else(|| date.split(' ').nth(1))
            .unwrap_or("      ");
        let time_display = &time_part[..5.min(time_part.len())];

        println!(
            "    {d}{time}{r} {c}→{r} {b}{owner}/{name}{r} v{g}{ver}{r}",
            d = crate::color::dim(),
            time = time_display,
            r = crate::color::reset(),
            c = crate::color::cyan(),
            b = crate::color::bold(),
            g = crate::color::green(),
            ver = version,
        );
    }
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::registry::db::Database;

    #[test]
    fn test_timeline_with_empty_db() {
        let db = Database::open_in_memory().unwrap();
        let versions = db.recent_versions(10).unwrap();
        assert!(versions.is_empty());
    }
}
