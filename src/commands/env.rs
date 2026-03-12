use crate::api::client::RegistryClient;
use crate::config::Config;
use crate::error::{McpRegError, Result};

/// Show environment variables required/used by an MCP server.
pub async fn run(server_ref: &str, json_output: bool) -> Result<()> {
    let parts: Vec<&str> = server_ref.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(McpRegError::Config(
            "Server reference must be in format 'owner/name'".into(),
        ));
    }
    let (owner, name) = (parts[0], parts[1]);

    // Try local DB first for env info from manifest
    let db_path = Config::db_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "registry.db".to_string());

    let entry = if let Ok(db) = crate::registry::db::Database::open(&db_path) {
        db.get_server(owner, name)?
    } else {
        None
    };

    let entry = match entry {
        Some(e) => e,
        None => {
            let config = Config::load()?;
            let client = RegistryClient::new(&config);
            client.get_server(owner, name).await?
        }
    };

    // Infer environment variables from well-known patterns
    let env_vars = infer_env_vars(&entry.owner, &entry.name, &entry.command, &entry.args);

    if json_output {
        let output = serde_json::json!({
            "server": entry.full_name(),
            "environment_variables": env_vars,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("Environment variables for {}:\n", entry.full_name());
    if env_vars.is_empty() {
        println!("  No known environment variables for this server.");
    } else {
        for (var, description) in &env_vars {
            println!("  {var}");
            println!("    {description}\n");
        }
    }

    Ok(())
}

/// Infer environment variables from well-known MCP server patterns.
fn infer_env_vars(owner: &str, name: &str, _command: &str, _args: &[String]) -> Vec<(String, String)> {
    let key = format!("{owner}/{name}");
    match key.as_str() {
        "modelcontextprotocol/github" => vec![
            ("GITHUB_PERSONAL_ACCESS_TOKEN".into(), "GitHub personal access token for API authentication".into()),
        ],
        "modelcontextprotocol/gitlab" => vec![
            ("GITLAB_PERSONAL_ACCESS_TOKEN".into(), "GitLab personal access token".into()),
            ("GITLAB_API_URL".into(), "GitLab API base URL (default: https://gitlab.com/api/v4)".into()),
        ],
        "modelcontextprotocol/slack" | "zencoderai/slack" => vec![
            ("SLACK_BOT_TOKEN".into(), "Slack Bot User OAuth Token (xoxb-...)".into()),
            ("SLACK_TEAM_ID".into(), "Slack workspace/team ID".into()),
        ],
        "modelcontextprotocol/postgres" => vec![
            ("POSTGRES_CONNECTION_STRING".into(), "PostgreSQL connection URI (postgresql://user:pass@host/db)".into()),
        ],
        "modelcontextprotocol/brave-search" | "brave/brave-search" => vec![
            ("BRAVE_API_KEY".into(), "Brave Search API key".into()),
        ],
        "modelcontextprotocol/gdrive" => vec![
            ("GOOGLE_CLIENT_ID".into(), "Google OAuth client ID".into()),
            ("GOOGLE_CLIENT_SECRET".into(), "Google OAuth client secret".into()),
        ],
        "modelcontextprotocol/google-maps" => vec![
            ("GOOGLE_MAPS_API_KEY".into(), "Google Maps API key".into()),
        ],
        "modelcontextprotocol/sentry" => vec![
            ("SENTRY_AUTH_TOKEN".into(), "Sentry authentication token".into()),
            ("SENTRY_ORG".into(), "Sentry organization slug".into()),
        ],
        "modelcontextprotocol/everart" => vec![
            ("EVERART_API_KEY".into(), "EverArt API key".into()),
        ],
        "modelcontextprotocol/redis" => vec![
            ("REDIS_URL".into(), "Redis connection URL (redis://host:port)".into()),
        ],
        "modelcontextprotocol/aws-kb-retrieval" => vec![
            ("AWS_ACCESS_KEY_ID".into(), "AWS access key".into()),
            ("AWS_SECRET_ACCESS_KEY".into(), "AWS secret key".into()),
            ("AWS_REGION".into(), "AWS region".into()),
        ],
        "tavily/tavily" => vec![
            ("TAVILY_API_KEY".into(), "Tavily API key".into()),
        ],
        "exa/exa" => vec![
            ("EXA_API_KEY".into(), "Exa API key".into()),
        ],
        "stripe/stripe-agent-toolkit" => vec![
            ("STRIPE_SECRET_KEY".into(), "Stripe secret API key".into()),
        ],
        "linear/linear" => vec![
            ("LINEAR_API_KEY".into(), "Linear API key".into()),
        ],
        "supabase/supabase" => vec![
            ("SUPABASE_ACCESS_TOKEN".into(), "Supabase access token".into()),
        ],
        "cloudflare/cloudflare" => vec![
            ("CLOUDFLARE_API_TOKEN".into(), "Cloudflare API token".into()),
            ("CLOUDFLARE_ACCOUNT_ID".into(), "Cloudflare account ID".into()),
        ],
        "neon/neon" => vec![
            ("NEON_API_KEY".into(), "Neon API key".into()),
        ],
        "resend/resend" => vec![
            ("RESEND_API_KEY".into(), "Resend API key".into()),
        ],
        "browserbase/stagehand" => vec![
            ("BROWSERBASE_API_KEY".into(), "Browserbase API key".into()),
            ("BROWSERBASE_PROJECT_ID".into(), "Browserbase project ID".into()),
        ],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_env_vars_known_server() {
        let vars = infer_env_vars("modelcontextprotocol", "github", "npx", &[]);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].0, "GITHUB_PERSONAL_ACCESS_TOKEN");
    }

    #[test]
    fn test_infer_env_vars_unknown_server() {
        let vars = infer_env_vars("unknown", "server", "node", &[]);
        assert!(vars.is_empty());
    }

    #[test]
    fn test_infer_env_vars_stripe() {
        let vars = infer_env_vars("stripe", "stripe-agent-toolkit", "npx", &[]);
        assert_eq!(vars.len(), 1);
        assert!(vars[0].0.contains("STRIPE"));
    }

    #[test]
    fn test_infer_env_vars_aws() {
        let vars = infer_env_vars("modelcontextprotocol", "aws-kb-retrieval", "npx", &[]);
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn test_infer_env_vars_cloudflare() {
        let vars = infer_env_vars("cloudflare", "cloudflare", "npx", &[]);
        assert_eq!(vars.len(), 2);
        assert!(vars.iter().any(|(k, _)| k == "CLOUDFLARE_API_TOKEN"));
        assert!(vars.iter().any(|(k, _)| k == "CLOUDFLARE_ACCOUNT_ID"));
    }
}
