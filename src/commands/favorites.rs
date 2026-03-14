use crate::config::Config;
use crate::error::{McpRegError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const FAVORITES_FILE: &str = "favorites.json";

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Favorites {
    pub servers: BTreeMap<String, FavoriteEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FavoriteEntry {
    pub owner: String,
    pub name: String,
    pub added_at: String,
    #[serde(default)]
    pub note: String,
}

fn favorites_path() -> Result<std::path::PathBuf> {
    let dir = Config::config_dir()?;
    Ok(dir.join(FAVORITES_FILE))
}

fn load_favorites() -> Result<Favorites> {
    let path = favorites_path()?;
    if !path.exists() {
        return Ok(Favorites::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let favs: Favorites = serde_json::from_str(&content)?;
    Ok(favs)
}

fn save_favorites(favs: &Favorites) -> Result<()> {
    let path = favorites_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(favs)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Add a server to favorites
pub fn add(server_ref: &str, note: Option<&str>) -> Result<()> {
    let (owner, name) = parse_ref(server_ref)?;
    let mut favs = load_favorites()?;
    let key = format!("{owner}/{name}");

    let entry = FavoriteEntry {
        owner: owner.clone(),
        name: name.clone(),
        added_at: crate::commands::install::chrono_now_public(),
        note: note.unwrap_or("").to_string(),
    };

    favs.servers.insert(key.clone(), entry);
    save_favorites(&favs)?;

    println!("⭐ Added {key} to favorites");
    if let Some(n) = note {
        println!("   Note: {n}");
    }
    Ok(())
}

/// Remove a server from favorites
pub fn remove(server_ref: &str) -> Result<()> {
    let (owner, name) = parse_ref(server_ref)?;
    let mut favs = load_favorites()?;
    let key = format!("{owner}/{name}");

    if favs.servers.remove(&key).is_some() {
        save_favorites(&favs)?;
        println!("Removed {key} from favorites");
    } else {
        println!("{key} is not in your favorites");
    }
    Ok(())
}

/// List all favorites
pub fn list(json_output: bool) -> Result<()> {
    let favs = load_favorites()?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&favs)?);
        return Ok(());
    }

    if favs.servers.is_empty() {
        println!("No favorites yet. Add one with: mcpreg favorite add <owner/name>");
        return Ok(());
    }

    println!("⭐ Your favorites ({} server(s)):\n", favs.servers.len());
    for (key, entry) in &favs.servers {
        if entry.note.is_empty() {
            println!("  {key}  (added {})", entry.added_at);
        } else {
            println!("  {key}  — {}  (added {})", entry.note, entry.added_at);
        }
    }
    println!(
        "\nInstall all: {}",
        favs.servers
            .keys()
            .map(|k| format!("mcpreg install {k}"))
            .collect::<Vec<_>>()
            .join(" && ")
    );
    Ok(())
}

fn parse_ref(server_ref: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = server_ref.splitn(2, '/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(McpRegError::Config(
            "Server reference must be in format 'owner/name'".into(),
        ));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_favorites_default_empty() {
        let favs = Favorites::default();
        assert!(favs.servers.is_empty());
    }

    #[test]
    fn test_favorites_serde_roundtrip() {
        let mut favs = Favorites::default();
        favs.servers.insert(
            "alice/tool".into(),
            FavoriteEntry {
                owner: "alice".into(),
                name: "tool".into(),
                added_at: "2025-01-01T00:00:00Z".into(),
                note: "Great tool".into(),
            },
        );
        let json = serde_json::to_string(&favs).unwrap();
        let back: Favorites = serde_json::from_str(&json).unwrap();
        assert_eq!(back.servers.len(), 1);
        assert_eq!(back.servers["alice/tool"].note, "Great tool");
    }

    #[test]
    fn test_favorites_multiple_entries() {
        let mut favs = Favorites::default();
        for (owner, name) in [("a", "tool1"), ("b", "tool2"), ("c", "tool3")] {
            favs.servers.insert(
                format!("{owner}/{name}"),
                FavoriteEntry {
                    owner: owner.into(),
                    name: name.into(),
                    added_at: "2025-01-01".into(),
                    note: String::new(),
                },
            );
        }
        assert_eq!(favs.servers.len(), 3);
        // BTreeMap should be sorted
        let keys: Vec<_> = favs.servers.keys().collect();
        assert_eq!(keys, vec!["a/tool1", "b/tool2", "c/tool3"]);
    }

    #[test]
    fn test_favorites_note_default() {
        let json = r#"{"owner":"a","name":"b","added_at":"2025-01-01"}"#;
        let entry: FavoriteEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.note, "");
    }

    #[test]
    fn test_parse_ref_valid() {
        let (owner, name) = parse_ref("alice/tool").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(name, "tool");
    }

    #[test]
    fn test_parse_ref_invalid() {
        assert!(parse_ref("noslash").is_err());
        assert!(parse_ref("/noowner").is_err());
        assert!(parse_ref("noname/").is_err());
    }

    #[test]
    fn test_parse_ref_with_extra_slash() {
        let (owner, name) = parse_ref("org/sub/name").unwrap();
        assert_eq!(owner, "org");
        assert_eq!(name, "sub/name");
    }
}
