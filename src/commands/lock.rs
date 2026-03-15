use crate::api::types::InstalledServers;
use crate::config::Config;
use crate::error::{McpRegError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const LOCKFILE_NAME: &str = "mcpreg.lock.json";

/// A lockfile entry for a single server — pins version and command.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LockedServer {
    pub owner: String,
    pub name: String,
    pub version: String,
    pub command: String,
    pub args: Vec<String>,
    pub transport: String,
    /// SHA-256 hash of the serialized entry for integrity checking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

/// The complete lockfile.
#[derive(Debug, Serialize, Deserialize)]
pub struct Lockfile {
    /// Lockfile format version.
    pub lockfile_version: u32,
    /// When the lockfile was generated.
    pub generated_at: String,
    /// mcpreg version that generated this lockfile.
    pub generator: String,
    /// Locked servers, keyed by "owner/name".
    pub servers: BTreeMap<String, LockedServer>,
}

impl Lockfile {
    /// Create a lockfile from the currently installed servers.
    pub fn from_installed(installed: &InstalledServers) -> Self {
        let mut servers = BTreeMap::new();
        for s in &installed.servers {
            let key = format!("{}/{}", s.owner, s.name);
            let locked = LockedServer {
                owner: s.owner.clone(),
                name: s.name.clone(),
                version: s.version.clone(),
                command: s.command.clone(),
                args: s.args.clone(),
                transport: s.transport.clone(),
                checksum: Some(compute_checksum(&s.owner, &s.name, &s.version)),
            };
            servers.insert(key, locked);
        }

        Self {
            lockfile_version: 1,
            generated_at: chrono_now(),
            generator: format!("mcpreg v{}", env!("CARGO_PKG_VERSION")),
            servers,
        }
    }

    /// Verify that all locked servers match the currently installed versions.
    pub fn verify(&self, installed: &InstalledServers) -> Vec<LockDrift> {
        let mut drifts = Vec::new();
        let installed_map: BTreeMap<String, &crate::api::types::InstalledServer> = installed
            .servers
            .iter()
            .map(|s| (format!("{}/{}", s.owner, s.name), s))
            .collect();

        for (key, locked) in &self.servers {
            match installed_map.get(key) {
                None => drifts.push(LockDrift {
                    server: key.clone(),
                    kind: DriftKind::Missing,
                    expected: locked.version.clone(),
                    actual: String::new(),
                }),
                Some(inst) => {
                    if inst.version != locked.version {
                        drifts.push(LockDrift {
                            server: key.clone(),
                            kind: DriftKind::VersionMismatch,
                            expected: locked.version.clone(),
                            actual: inst.version.clone(),
                        });
                    }
                }
            }
        }

        // Check for extra installed servers not in lockfile
        for key in installed_map.keys() {
            if !self.servers.contains_key(key) {
                drifts.push(LockDrift {
                    server: key.clone(),
                    kind: DriftKind::Extra,
                    expected: String::new(),
                    actual: installed_map[key].version.clone(),
                });
            }
        }

        drifts
    }
}

#[derive(Debug, Serialize)]
pub struct LockDrift {
    pub server: String,
    pub kind: DriftKind,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Serialize)]
pub enum DriftKind {
    Missing,
    VersionMismatch,
    Extra,
}

impl std::fmt::Display for DriftKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriftKind::Missing => write!(f, "missing"),
            DriftKind::VersionMismatch => write!(f, "version mismatch"),
            DriftKind::Extra => write!(f, "extra (not in lockfile)"),
        }
    }
}

fn compute_checksum(owner: &str, name: &str, version: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    format!("{owner}/{name}@{version}").hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn chrono_now() -> String {
    // Simple ISO 8601 without chrono dependency
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}Z", dur.as_secs())
}

fn load_installed() -> Result<InstalledServers> {
    let path = Config::installed_servers_path()?;
    if !path.exists() {
        return Ok(InstalledServers::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let installed: InstalledServers = serde_json::from_str(&content)?;
    Ok(installed)
}

/// Generate a lockfile from currently installed servers.
pub fn run_generate(output: Option<&str>, json_output: bool) -> Result<()> {
    let installed = load_installed()?;
    if installed.servers.is_empty() {
        return Err(McpRegError::Config("No installed servers to lock".into()));
    }

    let lockfile = Lockfile::from_installed(&installed);
    let serialized = serde_json::to_string_pretty(&lockfile)?;

    match output {
        Some(path) => {
            std::fs::write(path, &serialized)?;
            if !json_output {
                println!("✅ Lockfile written to {path} ({} server(s))", lockfile.servers.len());
            }
        }
        None => {
            let path = LOCKFILE_NAME;
            std::fs::write(path, &serialized)?;
            if !json_output {
                println!("✅ Lockfile written to {path} ({} server(s))", lockfile.servers.len());
            }
        }
    }

    if json_output {
        println!("{serialized}");
    }

    Ok(())
}

/// Verify installed servers match the lockfile.
pub fn run_verify(lockfile_path: Option<&str>, json_output: bool) -> Result<()> {
    let path = lockfile_path.unwrap_or(LOCKFILE_NAME);
    if !std::path::Path::new(path).exists() {
        return Err(McpRegError::Config(format!(
            "Lockfile not found: {path}\nRun 'mcpreg lock generate' to create one."
        )));
    }

    let content = std::fs::read_to_string(path)?;
    let lockfile: Lockfile = serde_json::from_str(&content)?;
    let installed = load_installed()?;
    let drifts = lockfile.verify(&installed);

    if json_output {
        let result = serde_json::json!({
            "lockfile": path,
            "locked_servers": lockfile.servers.len(),
            "installed_servers": installed.servers.len(),
            "drifts": drifts.iter().map(|d| serde_json::json!({
                "server": d.server,
                "kind": format!("{}", d.kind),
                "expected": d.expected,
                "actual": d.actual,
            })).collect::<Vec<_>>(),
            "ok": drifts.is_empty(),
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    if drifts.is_empty() {
        println!("✅ All {} locked server(s) match installed versions.", lockfile.servers.len());
        return Ok(());
    }

    println!("⚠️  Drift detected ({} issue(s)):\n", drifts.len());
    for drift in &drifts {
        match drift.kind {
            DriftKind::Missing => {
                println!("  ❌ {} — locked at v{} but NOT installed", drift.server, drift.expected);
            }
            DriftKind::VersionMismatch => {
                println!("  ⚡ {} — locked v{} ≠ installed v{}", drift.server, drift.expected, drift.actual);
            }
            DriftKind::Extra => {
                println!("  ➕ {} v{} — installed but not in lockfile", drift.server, drift.actual);
            }
        }
    }

    println!("\nRun 'mcpreg lock generate' to update the lockfile.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{InstalledServer, InstalledServers};

    fn sample_installed() -> InstalledServers {
        InstalledServers {
            servers: vec![
                InstalledServer {
                    owner: "modelcontextprotocol".into(),
                    name: "filesystem".into(),
                    version: "1.0.0".into(),
                    command: "npx".into(),
                    args: vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into()],
                    transport: "stdio".into(),
                    installed_at: "2024-01-01".into(),
                },
                InstalledServer {
                    owner: "modelcontextprotocol".into(),
                    name: "sqlite".into(),
                    version: "2.1.0".into(),
                    command: "uvx".into(),
                    args: vec!["mcp-server-sqlite".into()],
                    transport: "stdio".into(),
                    installed_at: "2024-01-02".into(),
                },
            ],
        }
    }

    #[test]
    fn test_lockfile_from_installed() {
        let installed = sample_installed();
        let lockfile = Lockfile::from_installed(&installed);
        assert_eq!(lockfile.lockfile_version, 1);
        assert_eq!(lockfile.servers.len(), 2);
        assert!(lockfile.servers.contains_key("modelcontextprotocol/filesystem"));
        assert!(lockfile.servers.contains_key("modelcontextprotocol/sqlite"));

        let fs = &lockfile.servers["modelcontextprotocol/filesystem"];
        assert_eq!(fs.version, "1.0.0");
        assert_eq!(fs.command, "npx");
        assert!(fs.checksum.is_some());
    }

    #[test]
    fn test_lockfile_verify_all_match() {
        let installed = sample_installed();
        let lockfile = Lockfile::from_installed(&installed);
        let drifts = lockfile.verify(&installed);
        assert!(drifts.is_empty(), "No drift expected when lockfile matches installed");
    }

    #[test]
    fn test_lockfile_verify_missing_server() {
        let installed = sample_installed();
        let lockfile = Lockfile::from_installed(&installed);

        // Remove one server from installed
        let partial = InstalledServers {
            servers: vec![installed.servers[0].clone()],
        };
        let drifts = lockfile.verify(&partial);
        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].server, "modelcontextprotocol/sqlite");
        assert!(matches!(drifts[0].kind, DriftKind::Missing));
    }

    #[test]
    fn test_lockfile_verify_version_mismatch() {
        let installed = sample_installed();
        let lockfile = Lockfile::from_installed(&installed);

        // Change version of installed server
        let mut modified = installed;
        modified.servers[0].version = "2.0.0".into();
        let drifts = lockfile.verify(&modified);
        assert_eq!(drifts.len(), 1);
        assert!(matches!(drifts[0].kind, DriftKind::VersionMismatch));
        assert_eq!(drifts[0].expected, "1.0.0");
        assert_eq!(drifts[0].actual, "2.0.0");
    }

    #[test]
    fn test_lockfile_verify_extra_server() {
        let installed = sample_installed();
        // Lock only one server
        let partial = InstalledServers {
            servers: vec![installed.servers[0].clone()],
        };
        let lockfile = Lockfile::from_installed(&partial);
        let drifts = lockfile.verify(&installed);
        assert_eq!(drifts.len(), 1);
        assert!(matches!(drifts[0].kind, DriftKind::Extra));
    }

    #[test]
    fn test_lockfile_serialization_roundtrip() {
        let installed = sample_installed();
        let lockfile = Lockfile::from_installed(&installed);
        let json = serde_json::to_string_pretty(&lockfile).unwrap();
        let deserialized: Lockfile = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.servers.len(), 2);
        assert_eq!(deserialized.lockfile_version, 1);
    }

    #[test]
    fn test_compute_checksum_deterministic() {
        let a = compute_checksum("owner", "name", "1.0.0");
        let b = compute_checksum("owner", "name", "1.0.0");
        assert_eq!(a, b);
        let c = compute_checksum("owner", "name", "2.0.0");
        assert_ne!(a, c);
    }

    #[test]
    fn test_lockfile_empty_installed() {
        let installed = InstalledServers::default();
        let lockfile = Lockfile::from_installed(&installed);
        assert!(lockfile.servers.is_empty());
    }

    #[test]
    fn test_drift_kind_display() {
        assert_eq!(format!("{}", DriftKind::Missing), "missing");
        assert_eq!(format!("{}", DriftKind::VersionMismatch), "version mismatch");
        assert_eq!(format!("{}", DriftKind::Extra), "extra (not in lockfile)");
    }

    #[test]
    fn test_lockfile_verify_empty_vs_empty() {
        let empty = InstalledServers::default();
        let lockfile = Lockfile::from_installed(&empty);
        let drifts = lockfile.verify(&empty);
        assert!(drifts.is_empty());
    }
}
