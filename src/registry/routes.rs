use crate::api::types::{PaginatedResponse, PublishResponse, SearchResponse, ServerEntry};
use crate::error::McpRegError;
#[allow(unused_imports)]
use crate::registry::db::Database;
use axum::extract::{Path, Query, State};
use axum::response::Json;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

pub type DbState = Arc<Mutex<Database>>;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub category: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<usize>,
    pub min_downloads: Option<i64>,
    pub tool: Option<String>,
    pub transport: Option<String>,
    pub author: Option<String>,
    pub owner: Option<String>,
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub page: Option<usize>,
    pub per_page: Option<usize>,
}

pub async fn search(
    State(db): State<DbState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, McpRegError> {
    let query = params.q.unwrap_or_default();
    let db = db.lock().await;
    let mut servers = db.search(&query)?;

    // Server-side category filter
    if let Some(ref cat) = params.category {
        let cat_lower = cat.to_lowercase();
        servers.retain(|s| {
            let server_cat = crate::registry::seed::server_category(&s.owner, &s.name).to_lowercase();
            server_cat.contains(&cat_lower)
        });
    }

    // Server-side min_downloads filter
    if let Some(min) = params.min_downloads {
        servers.retain(|s| s.downloads >= min);
    }

    // Server-side tool filter
    if let Some(ref tool) = params.tool {
        let tool_lower = tool.to_lowercase();
        servers.retain(|s| {
            s.tools.iter().any(|t| t.to_lowercase().contains(&tool_lower))
        });
    }

    // Server-side transport filter
    if let Some(ref transport) = params.transport {
        let t_lower = transport.to_lowercase();
        servers.retain(|s| s.transport.to_lowercase() == t_lower);
    }

    // Server-side author filter
    if let Some(ref author) = params.author {
        let author_lower = author.to_lowercase();
        servers.retain(|s| s.author.to_lowercase().contains(&author_lower));
    }

    // Server-side owner filter
    if let Some(ref owner) = params.owner {
        let owner_lower = owner.to_lowercase();
        servers.retain(|s| s.owner.to_lowercase().contains(&owner_lower));
    }

    // Server-side sorting
    if let Some(ref sort) = params.sort {
        match sort.as_str() {
            "name" => servers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            "updated" => servers.sort_by(|a, b| {
                let a_time = a.updated_at.as_deref().unwrap_or("");
                let b_time = b.updated_at.as_deref().unwrap_or("");
                b_time.cmp(a_time)
            }),
            _ => {} // "downloads" or default — already sorted
        }
    }

    // Server-side limit
    if let Some(n) = params.limit {
        servers.truncate(n);
    }

    let total = servers.len();
    Ok(Json(SearchResponse { servers, total }))
}

pub async fn get_server(
    State(db): State<DbState>,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Json<ServerEntry>, McpRegError> {
    let db = db.lock().await;
    // Track download on info fetch
    let _ = db.increment_downloads(&owner, &name);
    match db.get_server(&owner, &name)? {
        Some(entry) => Ok(Json(entry)),
        None => Err(McpRegError::NotFound(format!("{owner}/{name}"))),
    }
}

pub async fn delete_server(
    State(db): State<DbState>,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    // Note: In production, validate Authorization header matches the owner
    let db = db.lock().await;
    if db.delete_server(&owner, &name)? {
        Ok(Json(serde_json::json!({
            "success": true,
            "message": format!("Deleted {owner}/{name}")
        })))
    } else {
        Err(McpRegError::NotFound(format!("{owner}/{name}")))
    }
}

pub async fn publish(
    State(db): State<DbState>,
    Json(entry): Json<ServerEntry>,
) -> Result<Json<PublishResponse>, McpRegError> {
    // Validate required fields
    if entry.owner.is_empty() || entry.name.is_empty() {
        return Err(McpRegError::Validation("owner and name are required".into()));
    }
    if entry.command.is_empty() {
        return Err(McpRegError::Validation("command is required".into()));
    }
    if entry.version.is_empty() {
        return Err(McpRegError::Validation("version is required".into()));
    }
    // Basic semver check
    let parts: Vec<&str> = entry.version.split('.').collect();
    if parts.len() < 2 || parts.iter().any(|p| p.parse::<u64>().is_err()) {
        return Err(McpRegError::Validation(
            "version must be in semver format (e.g. 1.0.0)".into(),
        ));
    }
    // Validate transport
    let valid_transports = ["stdio", "sse", "streamable-http"];
    if !entry.transport.is_empty() && !valid_transports.contains(&entry.transport.as_str()) {
        return Err(McpRegError::Validation(format!(
            "transport '{}' is not recognized (expected: {})",
            entry.transport,
            valid_transports.join(", ")
        )));
    }

    let db = db.lock().await;
    db.upsert_server(&entry)?;
    Ok(Json(PublishResponse {
        success: true,
        message: format!("Published {}/{} v{}", entry.owner, entry.name, entry.version),
    }))
}

pub async fn list_servers(
    State(db): State<DbState>,
    Query(params): Query<PaginationQuery>,
) -> Result<Json<PaginatedResponse>, McpRegError> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let db = db.lock().await;
    let (servers, total) = db.list_servers(page, per_page)?;
    Ok(Json(PaginatedResponse {
        servers,
        page,
        per_page,
        total,
    }))
}

/// POST /api/v1/servers/:owner/:name/download — track a download without fetching full info
pub async fn track_download(
    State(db): State<DbState>,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    if db.increment_downloads(&owner, &name)? {
        Ok(Json(serde_json::json!({
            "success": true,
            "message": format!("Download tracked for {owner}/{name}")
        })))
    } else {
        Err(McpRegError::NotFound(format!("{owner}/{name}")))
    }
}

pub async fn health(
    State(db): State<DbState>,
) -> Json<serde_json::Value> {
    let db = db.lock().await;
    let server_count = db
        .stats()
        .map(|s| s.total_servers as i64)
        .unwrap_or(-1);
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "servers": server_count,
    }))
}

/// GET /api/v1/version — server version info
pub async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "mcpreg",
        "version": env!("CARGO_PKG_VERSION"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
    }))
}

/// GET /api/v1/stats — aggregate registry statistics
pub async fn stats(
    State(db): State<DbState>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let s = db.stats()?;
    Ok(Json(serde_json::json!({
        "total_servers": s.total_servers,
        "total_downloads": s.total_downloads,
        "unique_owners": s.unique_owners,
        "avg_tools": s.avg_tools,
        "top_servers": s.top_servers.iter().map(|(n, d)| serde_json::json!({"name": n, "downloads": d})).collect::<Vec<_>>(),
        "transports": s.transport_counts.iter().map(|(t, c)| serde_json::json!({"transport": t, "count": c})).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct CategoryQuery {
    pub category: Option<String>,
    pub page: Option<usize>,
    pub per_page: Option<usize>,
}

/// GET /api/v1/categories — list servers grouped or filtered by category
pub async fn categories(
    State(db): State<DbState>,
    Query(params): Query<CategoryQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    use crate::registry::seed::server_category;
    use std::collections::BTreeMap;

    let db = db.lock().await;
    let (servers, _) = db.list_servers(1, 1000)?;

    let mut by_cat: BTreeMap<String, Vec<&ServerEntry>> = BTreeMap::new();
    for s in &servers {
        let cat = server_category(&s.owner, &s.name).to_string();
        by_cat.entry(cat).or_default().push(s);
    }

    if let Some(ref filter) = params.category {
        let filter_lower = filter.to_lowercase();
        by_cat.retain(|k, _| k.to_lowercase().contains(&filter_lower));
    }

    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(50).min(200);

    let categories: Vec<serde_json::Value> = by_cat
        .iter()
        .map(|(cat, servers)| {
            serde_json::json!({
                "category": cat,
                "count": servers.len(),
                "servers": servers.iter().map(|s| s.full_name()).collect::<Vec<_>>(),
            })
        })
        .collect();

    let total = categories.len();
    let start = (page.saturating_sub(1)) * per_page;
    let page_items: Vec<_> = categories.into_iter().skip(start).take(per_page).collect();

    Ok(Json(serde_json::json!({
        "categories": page_items,
        "total": total,
        "page": page,
    })))
}

/// GET /api/v1/tools — list all unique tools across the registry
pub async fn tools_index(
    State(db): State<DbState>,
    Query(params): Query<ToolsQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let all_tools = db.list_tools()?;

    let mut items: Vec<serde_json::Value> = all_tools
        .into_iter()
        .map(|(tool, servers)| {
            serde_json::json!({
                "tool": tool,
                "server_count": servers.len(),
                "servers": servers,
            })
        })
        .collect();

    // Optional name filter
    if let Some(ref q) = params.q {
        let q_lower = q.to_lowercase();
        items.retain(|item| {
            item["tool"]
                .as_str()
                .map(|t| t.to_lowercase().contains(&q_lower))
                .unwrap_or(false)
        });
    }

    let total = items.len();
    let limit = params.limit.unwrap_or(100).min(500);
    items.truncate(limit);

    Ok(Json(serde_json::json!({
        "tools": items,
        "total": total,
    })))
}

#[derive(Deserialize)]
pub struct ToolsQuery {
    pub q: Option<String>,
    pub limit: Option<usize>,
}

/// POST /api/v1/servers/batch — fetch multiple servers by owner/name pairs
pub async fn batch_get_servers(
    State(db): State<DbState>,
    Json(body): Json<BatchRequest>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    if body.servers.is_empty() {
        return Ok(Json(serde_json::json!({"servers": [], "total": 0, "not_found": []})));
    }
    if body.servers.len() > 50 {
        return Err(McpRegError::Validation("Maximum 50 servers per batch request".into()));
    }

    let db = db.lock().await;
    let mut found = Vec::new();
    let mut not_found = Vec::new();

    for ref_str in &body.servers {
        let parts: Vec<&str> = ref_str.splitn(2, '/').collect();
        if parts.len() != 2 {
            not_found.push(ref_str.clone());
            continue;
        }
        match db.get_server(parts[0], parts[1])? {
            Some(entry) => found.push(entry),
            None => not_found.push(ref_str.clone()),
        }
    }

    let total = found.len();
    Ok(Json(serde_json::json!({
        "servers": found,
        "total": total,
        "not_found": not_found,
    })))
}

#[derive(Deserialize)]
pub struct BatchRequest {
    pub servers: Vec<String>,
}

/// GET /api/v1/prompts — list all unique prompts across the registry
pub async fn prompts_index(
    State(db): State<DbState>,
    Query(params): Query<ToolsQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let all_prompts = db.list_prompts()?;

    let mut items: Vec<serde_json::Value> = all_prompts
        .into_iter()
        .map(|(prompt, servers)| {
            serde_json::json!({
                "prompt": prompt,
                "server_count": servers.len(),
                "servers": servers,
            })
        })
        .collect();

    // Optional name filter
    if let Some(ref q) = params.q {
        let q_lower = q.to_lowercase();
        items.retain(|item| {
            item["prompt"]
                .as_str()
                .map(|p| p.to_lowercase().contains(&q_lower))
                .unwrap_or(false)
        });
    }

    let total = items.len();
    let limit = params.limit.unwrap_or(100).min(500);
    items.truncate(limit);

    Ok(Json(serde_json::json!({
        "prompts": items,
        "total": total,
    })))
}

#[derive(Deserialize)]
pub struct SimilarQuery {
    pub limit: Option<usize>,
}

/// GET /api/v1/servers/:owner/:name/versions — list version history
pub async fn version_history(
    State(db): State<DbState>,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    // Verify server exists
    match db.get_server(&owner, &name)? {
        Some(entry) => {
            let versions = db.get_version_history(&owner, &name)?;
            Ok(Json(serde_json::json!({
                "server": entry.full_name(),
                "current_version": entry.version,
                "versions": versions.iter().map(|(v, t)| serde_json::json!({
                    "version": v,
                    "published_at": t,
                })).collect::<Vec<_>>(),
                "total": versions.len(),
            })))
        }
        None => Err(McpRegError::NotFound(format!("{owner}/{name}"))),
    }
}

/// GET /api/v1/servers/:owner/:name/similar — find similar servers
pub async fn similar_servers(
    State(db): State<DbState>,
    Path((owner, name)): Path<(String, String)>,
    Query(params): Query<SimilarQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let limit = params.limit.unwrap_or(5).min(20);
    let db = db.lock().await;
    let similar = db.find_similar(&owner, &name, limit)?;
    let servers: Vec<serde_json::Value> = similar
        .iter()
        .map(|(entry, score)| {
            serde_json::json!({
                "owner": entry.owner,
                "name": entry.name,
                "full_name": entry.full_name(),
                "description": entry.description,
                "similarity_score": format!("{:.2}", score),
                "downloads": entry.downloads,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({
        "query": format!("{owner}/{name}"),
        "similar": servers,
        "total": servers.len(),
    })))
}

/// GET /api/v1/servers/:owner/:name/diff?from=VERSION
///
/// Compare the current server entry against a previous version.
/// Useful for checking what changed (new tools, updated description, etc.).
#[derive(Deserialize)]
pub struct DiffQuery {
    pub from: Option<String>,
}

pub async fn server_diff(
    State(db): State<DbState>,
    Path((owner, name)): Path<(String, String)>,
    Query(query): Query<DiffQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let current = db
        .get_server(&owner, &name)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner}/{name}")))?;

    let from_version = query.from.unwrap_or_default();

    // Build a basic diff — we show the current state and what the "from" version was
    // (In a full system we'd store snapshots; here we compare against version string)
    let versions = db.get_version_history(&owner, &name)?;
    let version_list: Vec<&str> = versions.iter().map(|(v, _)| v.as_str()).collect();

    let has_from = !from_version.is_empty() && version_list.contains(&from_version.as_str());

    Ok(Json(serde_json::json!({
        "server": format!("{owner}/{name}"),
        "current_version": current.version,
        "from_version": if has_from { &from_version } else { "unknown" },
        "current": {
            "tools": current.tools,
            "resources": current.resources,
            "prompts": current.prompts,
            "description": current.description,
            "transport": current.transport,
        },
        "versions": versions.iter().map(|(v, d)| serde_json::json!({"version": v, "date": d})).collect::<Vec<_>>(),
        "total_versions": versions.len(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_version_endpoint() {
        let result = version().await;
        let v = result.0;
        assert_eq!(v["name"], "mcpreg");
        assert!(!v["version"].as_str().unwrap().is_empty());
    }
}
