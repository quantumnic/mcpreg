use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

/// Rate an MCP server (1-5 stars) with an optional review comment.
pub fn run(server: &str, rating: u8, comment: Option<&str>, json: bool) -> Result<()> {
    if !(1..=5).contains(&rating) {
        return Err(crate::error::McpRegError::Validation(
            "Rating must be between 1 and 5".into(),
        ));
    }

    let parts: Vec<&str> = server.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(crate::error::McpRegError::Validation(
            "Server must be in 'owner/name' format".into(),
        ));
    }
    let (owner, name) = (parts[0], parts[1]);

    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());
    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    // Verify server exists
    if db.get_server(owner, name)?.is_none() {
        return Err(crate::error::McpRegError::NotFound(server.to_string()));
    }

    db.add_rating(owner, name, rating, comment)?;

    let stats = db.get_rating_stats(owner, name)?;

    if json {
        println!("{}", serde_json::json!({
            "server": server,
            "your_rating": rating,
            "comment": comment.unwrap_or(""),
            "average_rating": format!("{:.1}", stats.0),
            "total_ratings": stats.1,
        }));
    } else {
        let stars: String = "★".repeat(rating as usize) + &"☆".repeat(5 - rating as usize);
        println!("Rated {server}: {stars} ({rating}/5)");
        if let Some(c) = comment {
            println!("  Comment: {c}");
        }
        println!(
            "  Average: {:.1}/5 ({} rating{})",
            stats.0,
            stats.1,
            if stats.1 == 1 { "" } else { "s" }
        );
    }

    Ok(())
}

/// Show ratings for a server.
pub fn show(server: &str, json: bool) -> Result<()> {
    let parts: Vec<&str> = server.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(crate::error::McpRegError::Validation(
            "Server must be in 'owner/name' format".into(),
        ));
    }
    let (owner, name) = (parts[0], parts[1]);

    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());
    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    let stats = db.get_rating_stats(owner, name)?;
    let reviews = db.get_ratings(owner, name, 10)?;

    if json {
        let reviews_json: Vec<serde_json::Value> = reviews
            .iter()
            .map(|(rating, comment, date)| {
                serde_json::json!({
                    "rating": rating,
                    "comment": comment,
                    "date": date,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "server": server,
            "average_rating": format!("{:.1}", stats.0),
            "total_ratings": stats.1,
            "reviews": reviews_json,
        }))?);
    } else {
        if stats.1 == 0 {
            println!("{server}: No ratings yet");
            return Ok(());
        }

        let avg_stars = "★".repeat(stats.0.round() as usize)
            + &"☆".repeat(5 - stats.0.round() as usize);
        println!(
            "{server}: {avg_stars} {:.1}/5 ({} rating{})\n",
            stats.0,
            stats.1,
            if stats.1 == 1 { "" } else { "s" }
        );

        if !reviews.is_empty() {
            println!("Recent reviews:");
            for (rating, comment, date) in &reviews {
                let stars = "★".repeat(*rating as usize) + &"☆".repeat(5 - *rating as usize);
                print!("  {stars} ({date})");
                if !comment.is_empty() {
                    print!(" — {comment}");
                }
                println!();
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_invalid_rating_too_low() {
        let result = run("test/server", 0, None, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_rate_invalid_rating_too_high() {
        let result = run("test/server", 6, None, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_rate_invalid_server_format() {
        let result = run("noowner", 3, None, false);
        assert!(result.is_err());
    }
}
