use crate::error::{McpRegError, Result};

/// Maximum length for an owner identifier (matches GitHub's username limit).
pub const MAX_OWNER_LEN: usize = 39;
/// Maximum length for a server name.
pub const MAX_NAME_LEN: usize = 100;

/// Validate a registry identifier (`owner` or `name`).
///
/// Identifiers are used as URL path segments, JSON config keys, and parts of
/// `owner/name` references, so they must be tightly constrained:
/// - non-empty and within the length limit
/// - ASCII alphanumeric, hyphen, or underscore only
/// - must not start or end with a hyphen or underscore
pub fn validate_identifier(field: &str, value: &str, max_len: usize) -> Result<()> {
    if value.is_empty() {
        return Err(McpRegError::Validation(format!("{field} is required")));
    }
    if value.len() > max_len {
        return Err(McpRegError::Validation(format!(
            "{field} must be at most {max_len} characters (got {})",
            value.len()
        )));
    }
    if let Some(pos) = value
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_'))
        .map(|(i, _)| i)
    {
        return Err(McpRegError::Validation(format!(
            "{field} contains invalid character at position {pos}: allowed characters are A-Z, a-z, 0-9, '-', '_'"
        )));
    }
    if matches!(value.as_bytes().first(), Some(b'-') | Some(b'_')) {
        return Err(McpRegError::Validation(format!(
            "{field} must not start with '-' or '_'"
        )));
    }
    if matches!(value.as_bytes().last(), Some(b'-') | Some(b'_')) {
        return Err(McpRegError::Validation(format!(
            "{field} must not end with '-' or '_'"
        )));
    }
    Ok(())
}

/// Validate an `owner`/`name` pair for a server entry.
pub fn validate_server_ref(owner: &str, name: &str) -> Result<()> {
    validate_identifier("owner", owner, MAX_OWNER_LEN)?;
    validate_identifier("name", name, MAX_NAME_LEN)?;
    Ok(())
}

/// Validate a semantic version string (`MAJOR.MINOR.PATCH`, all numeric).
///
/// Each component must be a non-negative integer without leading zeros;
/// components wider than u64 are rejected.
pub fn validate_version(version: &str) -> Result<()> {
    if version.is_empty() {
        return Err(McpRegError::Validation("version is required".into()));
    }
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return Err(McpRegError::Validation(
            "version must be in semver format (e.g. 1.0.0)".into(),
        ));
    }
    for part in &parts {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(McpRegError::Validation(
                "version must be in semver format (e.g. 1.0.0)".into(),
            ));
        }
        if part.len() > 1 && part.starts_with('0') {
            return Err(McpRegError::Validation(format!(
                "version component '{part}' must not have leading zeros"
            )));
        }
        if part.parse::<u64>().is_err() {
            return Err(McpRegError::Validation(
                "version component is too large".into(),
            ));
        }
    }
    Ok(())
}

/// Return a human-readable issue for an invalid identifier, or `None` if valid.
/// Used by endpoints that collect all problems instead of failing fast.
pub fn identifier_issue(field: &str, value: &str, max_len: usize) -> Option<String> {
    validate_identifier(field, value, max_len)
        .err()
        .map(|e| e.to_string())
        .map(|msg| {
            msg.strip_prefix("Validation error: ")
                .unwrap_or(&msg)
                .to_string()
        })
}

/// Return a human-readable issue for an invalid version, or `None` if valid.
pub fn version_issue(version: &str) -> Option<String> {
    validate_version(version)
        .err()
        .map(|e| e.to_string())
        .map(|msg| {
            msg.strip_prefix("Validation error: ")
                .unwrap_or(&msg)
                .to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_identifiers() {
        for ok in ["alice", "modelcontextprotocol", "21st-dev", "a", "A_b-9"] {
            assert!(
                validate_identifier("name", ok, MAX_NAME_LEN).is_ok(),
                "'{ok}' should be valid"
            );
        }
    }

    #[test]
    fn test_empty_identifier_rejected() {
        assert!(validate_identifier("name", "", MAX_NAME_LEN).is_err());
    }

    #[test]
    fn test_path_traversal_rejected() {
        assert!(validate_identifier("owner", "../etc", MAX_OWNER_LEN).is_err());
        assert!(validate_identifier("name", "..", MAX_NAME_LEN).is_err());
        assert!(validate_identifier("name", "a/b", MAX_NAME_LEN).is_err());
    }

    #[test]
    fn test_spaces_and_control_chars_rejected() {
        assert!(validate_identifier("name", "foo bar", MAX_NAME_LEN).is_err());
        assert!(validate_identifier("name", "foo\nbar", MAX_NAME_LEN).is_err());
        assert!(validate_identifier("name", "foo\u{0000}", MAX_NAME_LEN).is_err());
    }

    #[test]
    fn test_unicode_rejected() {
        assert!(validate_identifier("name", "café", MAX_NAME_LEN).is_err());
    }

    #[test]
    fn test_leading_trailing_separator_rejected() {
        assert!(validate_identifier("name", "-foo", MAX_NAME_LEN).is_err());
        assert!(validate_identifier("name", "foo-", MAX_NAME_LEN).is_err());
        assert!(validate_identifier("name", "_foo", MAX_NAME_LEN).is_err());
        assert!(validate_identifier("name", "foo_", MAX_NAME_LEN).is_err());
    }

    #[test]
    fn test_length_limit_enforced() {
        let long = "a".repeat(MAX_NAME_LEN + 1);
        assert!(validate_identifier("name", &long, MAX_NAME_LEN).is_err());
        let exact = "a".repeat(MAX_NAME_LEN);
        assert!(validate_identifier("name", &exact, MAX_NAME_LEN).is_ok());

        let long_owner = "o".repeat(MAX_OWNER_LEN + 1);
        assert!(validate_server_ref(&long_owner, "ok").is_err());
    }

    #[test]
    fn test_server_ref_validates_both_parts() {
        assert!(validate_server_ref("alice", "filesystem").is_ok());
        assert!(validate_server_ref("", "filesystem").is_err());
        assert!(validate_server_ref("alice", "").is_err());
        assert!(validate_server_ref("al ice", "filesystem").is_err());
    }

    #[test]
    fn test_valid_versions() {
        for ok in ["1.0.0", "2024.11.0", "0.0.1", "10.20.30"] {
            assert!(validate_version(ok).is_ok(), "'{ok}' should be valid");
        }
    }

    #[test]
    fn test_invalid_versions() {
        for bad in [
            "",
            "1.0",
            "1.0.0.0",
            "not-semver",
            "1.x.0",
            "-1.0.0",
            "1..0",
            "01.0.0",
            "1.0.01",
            "99999999999999999999999999.0.0",
        ] {
            assert!(validate_version(bad).is_err(), "'{bad}' should be invalid");
        }
    }

    #[test]
    fn test_issue_helpers_strip_prefix() {
        let issue = identifier_issue("owner", "../etc", MAX_OWNER_LEN);
        assert!(issue.is_some());
        assert!(!issue.unwrap().starts_with("Validation error"));

        assert!(identifier_issue("owner", "alice", MAX_OWNER_LEN).is_none());
        assert!(version_issue("1.2.3").is_none());
        assert!(version_issue("bad").is_some());
    }

    #[test]
    fn test_seed_data_conforms() {
        // Every seeded server must pass validation, or publishing rules would
        // reject data shipped by default.
        for entry in crate::registry::seed::default_servers() {
            assert!(
                validate_server_ref(&entry.owner, &entry.name).is_ok(),
                "seeded {}/{} should conform",
                entry.owner,
                entry.name
            );
            assert!(
                validate_version(&entry.version).is_ok(),
                "seeded version {} of {}/{} should conform",
                entry.version,
                entry.owner,
                entry.name
            );
        }
    }
}
