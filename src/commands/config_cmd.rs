use crate::config::Config;
use crate::error::{McpRegError, Result};

pub fn run(action: &str, key: Option<&str>, value: Option<&str>) -> Result<()> {
    match action {
        "show" => show_config(),
        "get" => {
            let key = key.ok_or_else(|| McpRegError::Config("Key required for 'config get'".into()))?;
            get_config(key)
        }
        "set" => {
            let key = key.ok_or_else(|| McpRegError::Config("Key required for 'config set'".into()))?;
            let value = value.ok_or_else(|| McpRegError::Config("Value required for 'config set'".into()))?;
            set_config(key, value)
        }
        "path" => {
            println!("{}", Config::config_path()?.display());
            Ok(())
        }
        other => Err(McpRegError::Config(format!(
            "Unknown config action '{other}'. Use: show, get, set, path"
        ))),
    }
}

fn show_config() -> Result<()> {
    let config = Config::load()?;
    println!("mcpreg configuration:\n");
    println!("  registry_url  = {}", config.registry_url);
    println!(
        "  api_key       = {}",
        config.api_key.as_deref().map(mask_key).unwrap_or_else(|| "(not set)".into())
    );
    println!(
        "  install_dir   = {}",
        config.install_dir.as_deref().unwrap_or("(default)")
    );
    println!("\nConfig file: {}", Config::config_path()?.display());
    println!("Data dir:    {}", Config::config_dir()?.display());
    Ok(())
}

fn get_config(key: &str) -> Result<()> {
    let config = Config::load()?;
    match key {
        "registry_url" => println!("{}", config.registry_url),
        "api_key" => println!("{}", config.api_key.unwrap_or_default()),
        "install_dir" => println!("{}", config.install_dir.unwrap_or_default()),
        _ => return Err(McpRegError::Config(format!(
            "Unknown key '{key}'. Available: registry_url, api_key, install_dir"
        ))),
    }
    Ok(())
}

fn set_config(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load()?;
    match key {
        "registry_url" => config.registry_url = value.to_string(),
        "api_key" => config.api_key = Some(value.to_string()),
        "install_dir" => config.install_dir = Some(value.to_string()),
        _ => return Err(McpRegError::Config(format!(
            "Unknown key '{key}'. Available: registry_url, api_key, install_dir"
        ))),
    }
    config.save()?;
    println!("✓ Set {key} = {}", if key == "api_key" { mask_key(value) } else { value.to_string() });
    Ok(())
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_key_short() {
        assert_eq!(mask_key("abc"), "****");
    }

    #[test]
    fn test_mask_key_long() {
        assert_eq!(mask_key("abcdefghijklmnop"), "abcd...mnop");
    }

    #[test]
    fn test_config_show() {
        // Should not panic
        let _ = show_config();
    }

    #[test]
    fn test_config_path() {
        assert!(run("path", None, None).is_ok());
    }

    #[test]
    fn test_config_get_registry_url() {
        assert!(run("get", Some("registry_url"), None).is_ok());
    }

    #[test]
    fn test_config_get_unknown_key() {
        assert!(run("get", Some("nonexistent"), None).is_err());
    }

    #[test]
    fn test_config_unknown_action() {
        assert!(run("bogus", None, None).is_err());
    }

    #[test]
    fn test_config_set_requires_value() {
        assert!(run("set", Some("registry_url"), None).is_err());
    }
}
