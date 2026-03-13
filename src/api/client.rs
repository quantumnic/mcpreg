use crate::api::types::{PaginatedResponse, PublishResponse, SearchResponse, ServerEntry};
use crate::config::Config;
use crate::error::{McpRegError, Result};
use std::time::Duration;

/// Default request timeout in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Maximum number of retries for transient failures.
const MAX_RETRIES: u32 = 2;

/// Base delay between retries (doubles each attempt).
const RETRY_BASE_MS: u64 = 500;

pub struct RegistryClient {
    base_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
    max_retries: u32,
}

impl RegistryClient {
    pub fn new(config: &Config) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            base_url: config.registry_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
            client,
            max_retries: MAX_RETRIES,
        }
    }

    #[allow(dead_code)]
    pub fn with_base_url(base_url: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: None,
            client,
            max_retries: MAX_RETRIES,
        }
    }

    /// Execute a GET request with retry logic for transient failures.
    async fn get_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            match self.client.get(url).send().await {
                Ok(resp) => {
                    // Retry on 502/503/504 (transient server errors)
                    if is_retryable_status(resp.status()) && attempt < self.max_retries {
                        last_err = Some(McpRegError::Registry(format!(
                            "HTTP {} (attempt {}/{})",
                            resp.status(),
                            attempt + 1,
                            self.max_retries + 1
                        )));
                        tokio::time::sleep(retry_delay(attempt)).await;
                        continue;
                    }
                    return Ok(resp);
                }
                Err(e) => {
                    if is_retryable_error(&e) && attempt < self.max_retries {
                        last_err = Some(e.into());
                        tokio::time::sleep(retry_delay(attempt)).await;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }
        Err(last_err.unwrap_or_else(|| McpRegError::Registry("Request failed after retries".into())))
    }

    pub async fn search(&self, query: &str) -> Result<SearchResponse> {
        let url = format!("{}/api/v1/search?q={}", self.base_url, percent_encode(query));
        let resp = self.get_with_retry(&url).await?;
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
        let resp = self.get_with_retry(&url).await?;
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

    /// Check registry health / connectivity.
    pub async fn health(&self) -> Result<std::collections::HashMap<String, serde_json::Value>> {
        let url = format!("{}/health", self.base_url);
        let resp = self.get_with_retry(&url).await?;
        if !resp.status().is_success() {
            return Err(McpRegError::Registry(format!("Health check failed: HTTP {}", resp.status())));
        }
        let body = resp.json::<std::collections::HashMap<String, serde_json::Value>>().await?;
        Ok(body)
    }

    #[allow(dead_code)]
    pub async fn list_servers(&self, page: usize, per_page: usize) -> Result<PaginatedResponse> {
        let url = format!("{}/api/v1/servers?page={}&per_page={}", self.base_url, page, per_page);
        let resp = self.get_with_retry(&url).await?;
        if !resp.status().is_success() {
            return Err(McpRegError::Registry(format!("List failed: HTTP {}", resp.status())));
        }
        let body = resp.json::<PaginatedResponse>().await?;
        Ok(body)
    }

    /// Suggest server names matching a prefix (autocomplete).
    #[allow(dead_code)]
    pub async fn suggest(&self, prefix: &str, limit: usize) -> Result<Vec<String>> {
        let url = format!(
            "{}/api/v1/suggest?q={}&limit={}",
            self.base_url,
            percent_encode(prefix),
            limit
        );
        let resp = self.get_with_retry(&url).await?;
        if !resp.status().is_success() {
            return Err(McpRegError::Registry(format!("Suggest failed: HTTP {}", resp.status())));
        }
        let body: serde_json::Value = resp.json().await?;
        let suggestions = body["suggestions"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        Ok(suggestions)
    }
}

/// Check if an HTTP status code is retryable (transient server error).
fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(
        status,
        reqwest::StatusCode::BAD_GATEWAY
            | reqwest::StatusCode::SERVICE_UNAVAILABLE
            | reqwest::StatusCode::GATEWAY_TIMEOUT
            | reqwest::StatusCode::REQUEST_TIMEOUT
    )
}

/// Check if a reqwest error is retryable (timeout or connection issue).
fn is_retryable_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect()
}

/// Exponential backoff delay for retries.
fn retry_delay(attempt: u32) -> Duration {
    Duration::from_millis(RETRY_BASE_MS * 2u64.pow(attempt))
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
        assert_eq!(client.max_retries, MAX_RETRIES);
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

    #[test]
    fn test_is_retryable_status() {
        assert!(is_retryable_status(reqwest::StatusCode::BAD_GATEWAY));
        assert!(is_retryable_status(reqwest::StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_retryable_status(reqwest::StatusCode::GATEWAY_TIMEOUT));
        assert!(is_retryable_status(reqwest::StatusCode::REQUEST_TIMEOUT));
        assert!(!is_retryable_status(reqwest::StatusCode::OK));
        assert!(!is_retryable_status(reqwest::StatusCode::NOT_FOUND));
        assert!(!is_retryable_status(reqwest::StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(reqwest::StatusCode::INTERNAL_SERVER_ERROR));
    }

    #[test]
    fn test_retry_delay_exponential() {
        assert_eq!(retry_delay(0), Duration::from_millis(500));
        assert_eq!(retry_delay(1), Duration::from_millis(1000));
        assert_eq!(retry_delay(2), Duration::from_millis(2000));
    }
}
