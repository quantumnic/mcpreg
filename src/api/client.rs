use crate::api::types::{PaginatedResponse, PublishResponse, SearchResponse, ServerEntry};
use crate::config::Config;
use crate::error::{McpRegError, Result};

pub struct RegistryClient {
    base_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl RegistryClient {
    pub fn new(config: &Config) -> Self {
        Self {
            base_url: config.registry_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
            client: reqwest::Client::new(),
        }
    }

    #[allow(dead_code)]
    pub fn with_base_url(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: None,
            client: reqwest::Client::new(),
        }
    }

    pub async fn search(&self, query: &str) -> Result<SearchResponse> {
        let url = format!("{}/api/v1/search?q={}", self.base_url, percent_encode(query));
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(McpRegError::Registry(format!("Search failed: HTTP {}", resp.status())));
        }
        let body = resp.json::<SearchResponse>().await?;
        Ok(body)
    }

    pub async fn get_server(&self, owner: &str, name: &str) -> Result<ServerEntry> {
        let url = format!(
            "{}/api/v1/servers/{}/{}",
            self.base_url,
            percent_encode(owner),
            percent_encode(name)
        );
        let resp = self.client.get(&url).send().await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(McpRegError::NotFound(format!("{owner}/{name}")));
        }
        if !resp.status().is_success() {
            return Err(McpRegError::Registry(format!("Failed to get server: HTTP {}", resp.status())));
        }
        let entry = resp.json::<ServerEntry>().await?;
        Ok(entry)
    }

    pub async fn publish(&self, entry: &ServerEntry) -> Result<PublishResponse> {
        let api_key = self.api_key.as_ref().ok_or_else(|| {
            McpRegError::Auth("API key required for publishing. Set it in ~/.mcpreg/config.toml or via 'mcpreg config set api_key <key>'".into())
        })?;
        let url = format!("{}/api/v1/publish", self.base_url);
        let resp = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(entry)
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(McpRegError::Auth("Invalid API key".into()));
        }
        if !resp.status().is_success() {
            return Err(McpRegError::Registry(format!("Publish failed: HTTP {}", resp.status())));
        }
        let body = resp.json::<PublishResponse>().await?;
        Ok(body)
    }

    #[allow(dead_code)]
    pub async fn list_servers(&self, page: usize, per_page: usize) -> Result<PaginatedResponse> {
        let url = format!("{}/api/v1/servers?page={}&per_page={}", self.base_url, page, per_page);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(McpRegError::Registry(format!("List failed: HTTP {}", resp.status())));
        }
        let body = resp.json::<PaginatedResponse>().await?;
        Ok(body)
    }
}

/// Percent-encode a string for use in URLs (RFC 3986 unreserved chars only).
fn percent_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let config = Config::default();
        let client = RegistryClient::new(&config);
        assert_eq!(client.base_url, "https://registry.mcpreg.dev");
        assert!(client.api_key.is_none());
    }

    #[test]
    fn test_client_with_base_url() {
        let client = RegistryClient::with_base_url("http://localhost:3000/");
        assert_eq!(client.base_url, "http://localhost:3000");
    }

    #[test]
    fn test_percent_encode_simple() {
        assert_eq!(percent_encode("hello"), "hello");
    }

    #[test]
    fn test_percent_encode_spaces() {
        assert_eq!(percent_encode("hello world"), "hello%20world");
    }

    #[test]
    fn test_percent_encode_special() {
        assert_eq!(percent_encode("a/b"), "a%2Fb");
        assert_eq!(percent_encode("a&b=c"), "a%26b%3Dc");
        assert_eq!(percent_encode("q?x"), "q%3Fx");
    }

    #[test]
    fn test_percent_encode_unreserved() {
        // These should NOT be encoded
        assert_eq!(percent_encode("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn test_percent_encode_unicode() {
        let encoded = percent_encode("über");
        assert!(encoded.contains("%C3%BC")); // ü in UTF-8
    }
}
