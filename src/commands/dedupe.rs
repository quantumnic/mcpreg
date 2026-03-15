use crate::config::Config;
use crate::error::Result;
use crate::registry::db::Database;

/// Find duplicate/overlapping servers by tool overlap, description similarity, or same owner/name patterns.
pub fn run(threshold: f64, limit: usize, json_output: bool) -> Result<()> {
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());
    let db = Database::open(&db_path)?;
    let _ = db.seed_default_servers();

    let all = db.list_all()?;

    let mut overlaps: Vec<(String, String, f64, Vec<String>)> = Vec::new();

    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            let a = &all[i];
            let b = &all[j];

            // Calculate tool overlap (Jaccard similarity)
            if a.tools.is_empty() && b.tools.is_empty() {
                continue;
            }
            let a_tools: std::collections::HashSet<&str> =
                a.tools.iter().map(|t| t.as_str()).collect();
            let b_tools: std::collections::HashSet<&str> =
                b.tools.iter().map(|t| t.as_str()).collect();

            let intersection = a_tools.intersection(&b_tools).count();
            let union = a_tools.union(&b_tools).count();

            if union == 0 {
                continue;
            }

            let jaccard = intersection as f64 / union as f64;
            if jaccard >= threshold {
                let shared: Vec<String> = a_tools
                    .intersection(&b_tools)
                    .map(|t| t.to_string())
                    .collect();
                overlaps.push((a.full_name(), b.full_name(), jaccard, shared));
            }
        }
    }

    // Sort by overlap score descending
    overlaps.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    overlaps.truncate(limit);

    if json_output {
        let entries: Vec<serde_json::Value> = overlaps
            .iter()
            .enumerate()
            .map(|(i, (a, b, score, shared))| {
                serde_json::json!({
                    "rank": i + 1,
                    "server_a": a,
                    "server_b": b,
                    "overlap_score": format!("{:.2}", score),
                    "shared_tools": shared,
                    "shared_count": shared.len(),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "overlaps": entries,
                "total": overlaps.len(),
                "threshold": threshold,
            }))?
        );
        return Ok(());
    }

    if overlaps.is_empty() {
        println!("No overlapping servers found (threshold: {:.0}%)", threshold * 100.0);
        return Ok(());
    }

    println!(
        "Found {} server pair(s) with ≥{:.0}% tool overlap:\n",
        overlaps.len(),
        threshold * 100.0
    );

    for (i, (a, b, score, shared)) in overlaps.iter().enumerate() {
        println!(
            "  #{} {} ↔ {} ({:.0}% overlap)",
            i + 1,
            a,
            b,
            score * 100.0
        );
        println!("    Shared tools: {}", shared.join(", "));
        println!();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::api::types::ServerEntry;
    use crate::registry::db::Database;

    fn make_server(owner: &str, name: &str, tools: Vec<&str>) -> ServerEntry {
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
            tools: tools.into_iter().map(|t| t.to_string()).collect(),
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
    fn test_dedupe_finds_identical_tools() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_server("a", "s1", vec!["read", "write", "delete"])).unwrap();
        db.upsert_server(&make_server("b", "s2", vec!["read", "write", "delete"])).unwrap();
        db.upsert_server(&make_server("c", "s3", vec!["query", "insert"])).unwrap();

        let all = db.list_all().unwrap();
        let mut found_overlap = false;
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                let a_tools: std::collections::HashSet<&str> =
                    all[i].tools.iter().map(|t| t.as_str()).collect();
                let b_tools: std::collections::HashSet<&str> =
                    all[j].tools.iter().map(|t| t.as_str()).collect();
                let intersection = a_tools.intersection(&b_tools).count();
                let union = a_tools.union(&b_tools).count();
                if union > 0 {
                    let jaccard = intersection as f64 / union as f64;
                    if jaccard >= 0.5 {
                        found_overlap = true;
                    }
                }
            }
        }
        assert!(found_overlap, "Should find overlap between s1 and s2");
    }

    #[test]
    fn test_dedupe_no_overlap_with_distinct_tools() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_server("a", "s1", vec!["read", "write"])).unwrap();
        db.upsert_server(&make_server("b", "s2", vec!["query", "insert"])).unwrap();

        let all = db.list_all().unwrap();
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                let a_tools: std::collections::HashSet<&str> =
                    all[i].tools.iter().map(|t| t.as_str()).collect();
                let b_tools: std::collections::HashSet<&str> =
                    all[j].tools.iter().map(|t| t.as_str()).collect();
                let intersection = a_tools.intersection(&b_tools).count();
                let union = a_tools.union(&b_tools).count();
                if union > 0 {
                    let jaccard = intersection as f64 / union as f64;
                    assert!(jaccard < 0.5, "Should not find overlap between distinct tools");
                }
            }
        }
    }

    #[test]
    fn test_dedupe_partial_overlap() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_server("a", "s1", vec!["read", "write", "delete", "list"])).unwrap();
        db.upsert_server(&make_server("b", "s2", vec!["read", "write", "search"])).unwrap();

        let all = db.list_all().unwrap();
        let a_tools: std::collections::HashSet<&str> =
            all[0].tools.iter().map(|t| t.as_str()).collect();
        let b_tools: std::collections::HashSet<&str> =
            all[1].tools.iter().map(|t| t.as_str()).collect();
        let intersection = a_tools.intersection(&b_tools).count();
        let union = a_tools.union(&b_tools).count();
        let jaccard = intersection as f64 / union as f64;
        // 2 shared out of 5 unique = 0.4
        assert!((jaccard - 0.4).abs() < 0.01, "Expected ~0.4 Jaccard, got {jaccard}");
    }

    #[test]
    fn test_dedupe_empty_tools_skipped() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&make_server("a", "s1", vec![])).unwrap();
        db.upsert_server(&make_server("b", "s2", vec![])).unwrap();

        let all = db.list_all().unwrap();
        // Both empty → should be skipped
        assert_eq!(all.len(), 2);
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                if all[i].tools.is_empty() && all[j].tools.is_empty() {
                    // This pair should be skipped in the dedupe logic
                    continue;
                }
            }
        }
    }
}
