use crate::api::client::RegistryClient;
use crate::api::types::InstalledServers;
use crate::config::Config;
use crate::error::Result;

/// Watch installed servers for available updates (one-shot check with detailed output).
pub async fn run(json_output: bool) -> Result<()> {
    let config = Config::load()?;
    let installed_path = Config::installed_servers_path()?;

    if !installed_path.exists() {
        if json_output {
            println!("{{\"updates\":[]}}");
        } else {
            println!("No installed servers found. Install some with 'mcpreg install'.");
        }
        return Ok(());
    }

    let content = std::fs::read_to_string(&installed_path)?;
    let installed: InstalledServers = serde_json::from_str(&content)?;

    if installed.servers.is_empty() {
        if json_output {
            println!("{{\"updates\":[]}}");
        } else {
            println!("No installed servers to watch.");
        }
        return Ok(());
    }

    let client = RegistryClient::new(&config);
    let mut updates = Vec::new();
    let mut errors = Vec::new();

    for server in &installed.servers {
        match client.get_server(&server.owner, &server.name).await {
            Ok(remote) => {
                if crate::compare_versions(&remote.version, &server.version)
                    == std::cmp::Ordering::Greater
                {
                    updates.push(UpdateInfo {
                        name: server.full_name(),
                        current_version: server.version.clone(),
                        latest_version: remote.version.clone(),
                        description: remote.description.clone(),
                    });
                }
            }
            Err(e) => {
                errors.push(format!("{}: {e}", server.full_name()));
            }
        }
    }

    if json_output {
        let output = serde_json::json!({
            "updates": updates.iter().map(|u| serde_json::json!({
                "name": u.name,
                "current": u.current_version,
                "latest": u.latest_version,
                "description": u.description,
            })).collect::<Vec<_>>(),
            "checked": installed.servers.len(),
            "errors": errors,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!(
        "Checked {} installed server(s):\n",
        installed.servers.len()
    );

    if updates.is_empty() {
        println!("  ✅ All servers are up to date!");
    } else {
        println!("  {} update(s) available:\n", updates.len());
        for u in &updates {
            println!(
                "  📦 {b}{name}{r} {d}{current}{r} → {g}{latest}{r} ({desc})",
                b = crate::color::bold(),
                name = u.name,
                r = crate::color::reset(),
                d = crate::color::dim(),
                current = u.current_version,
                g = crate::color::green(),
                latest = u.latest_version,
                desc = u.description,
            );
        }
        println!("\n  Run 'mcpreg update' to apply all updates.");
    }

    if !errors.is_empty() {
        println!("\n  ⚠️  Could not check {} server(s):", errors.len());
        for err in &errors {
            println!("    • {err}");
        }
    }

    Ok(())
}

struct UpdateInfo {
    name: String,
    current_version: String,
    latest_version: String,
    description: String,
}
