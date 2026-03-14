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
pub struct LimitQuery {
    pub limit: Option<usize>,
}

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
    pub tag: Option<String>,
    pub min_tools: Option<usize>,
    pub has_prompts: Option<bool>,
    pub resource: Option<String>,
    /// Exclude deprecated servers from results (default: false)
    pub exclude_deprecated: Option<bool>,
    /// Filter by license (e.g. "MIT", "Apache")
    pub license: Option<String>,
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

    // Server-side tag filter
    if let Some(ref tag) = params.tag {
        let tag_lower = tag.to_lowercase();
        servers.retain(|s| {
            s.tags.iter().any(|t| t.to_lowercase().contains(&tag_lower))
        });
    }

    // Server-side min_tools filter
    if let Some(min) = params.min_tools {
        servers.retain(|s| s.tools.len() >= min);
    }

    // Server-side has_prompts filter
    if let Some(true) = params.has_prompts {
        servers.retain(|s| !s.prompts.is_empty());
    }

    // Server-side resource filter
    if let Some(ref resource) = params.resource {
        let r_lower = resource.to_lowercase();
        servers.retain(|s| {
            s.resources.iter().any(|r| r.to_lowercase().contains(&r_lower))
        });
    }

    // Exclude deprecated servers when requested
    if params.exclude_deprecated.unwrap_or(false) {
        servers.retain(|s| !s.deprecated);
    }

    // Server-side license filter
    if let Some(ref license) = params.license {
        let lic_lower = license.to_lowercase();
        servers.retain(|s| s.license.to_lowercase().contains(&lic_lower));
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

    // If zero results and non-empty query, add fuzzy suggestions
    if total == 0 && !query.is_empty() {
        let all_servers = db.list_servers(1, 500).unwrap_or_default();
        let names: Vec<String> = all_servers.0.iter().map(|s| s.full_name()).collect();
        let suggestions = crate::fuzzy::suggest(&query, &names, 4);
        if !suggestions.is_empty() {
            let suggestion_names: Vec<String> = suggestions.into_iter().map(|(n, _)| n).collect();
            return Ok(Json(SearchResponse {
                servers: vec![],
                total: 0,
                suggestions: Some(suggestion_names),
            }));
        }
    }

    Ok(Json(SearchResponse { servers, total, suggestions: None }))
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
        .count_servers()
        .map(|c| c as i64)
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

    // Extended stats: category breakdown, license distribution, capability totals
    let all_servers = db.list_all()?;
    let mut categories: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut license_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut total_tools = 0usize;
    let mut total_prompts = 0usize;
    let mut total_resources = 0usize;
    let mut deprecated_count = 0usize;

    for srv in &all_servers {
        let cat = crate::registry::seed::server_category(&srv.owner, &srv.name).to_string();
        *categories.entry(cat).or_insert(0) += 1;
        if !srv.license.is_empty() {
            *license_counts.entry(srv.license.clone()).or_insert(0) += 1;
        }
        total_tools += srv.tools.len();
        total_prompts += srv.prompts.len();
        total_resources += srv.resources.len();
        if srv.deprecated {
            deprecated_count += 1;
        }
    }

    let mut cats_sorted: Vec<_> = categories.into_iter().collect();
    cats_sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let mut licenses_sorted: Vec<_> = license_counts.into_iter().collect();
    licenses_sorted.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(Json(serde_json::json!({
        "total_servers": s.total_servers,
        "total_downloads": s.total_downloads,
        "unique_owners": s.unique_owners,
        "avg_tools": s.avg_tools,
        "total_tools": total_tools,
        "total_prompts": total_prompts,
        "total_resources": total_resources,
        "deprecated_servers": deprecated_count,
        "top_servers": s.top_servers.iter().map(|(n, d)| serde_json::json!({"name": n, "downloads": d})).collect::<Vec<_>>(),
        "transports": s.transport_counts.iter().map(|(t, c)| serde_json::json!({"transport": t, "count": c})).collect::<Vec<_>>(),
        "categories": cats_sorted.iter().map(|(c, n)| serde_json::json!({"category": c, "count": n})).collect::<Vec<_>>(),
        "licenses": licenses_sorted.iter().take(10).map(|(l, c)| serde_json::json!({"license": l, "count": c})).collect::<Vec<_>>(),
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

/// GET /api/v1/tags — list all unique tags across the registry
pub async fn tags_index(
    State(db): State<DbState>,
    Query(params): Query<ToolsQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let all_tags = db.list_tags()?;

    let mut items: Vec<serde_json::Value> = all_tags
        .into_iter()
        .map(|(tag, servers)| {
            serde_json::json!({
                "tag": tag,
                "server_count": servers.len(),
                "servers": servers,
            })
        })
        .collect();

    if let Some(ref q) = params.q {
        let q_lower = q.to_lowercase();
        items.retain(|item| {
            item["tag"]
                .as_str()
                .map(|t| t.to_lowercase().contains(&q_lower))
                .unwrap_or(false)
        });
    }

    let total = items.len();
    let limit = params.limit.unwrap_or(100).min(500);
    items.truncate(limit);

    Ok(Json(serde_json::json!({
        "tags": items,
        "total": total,
    })))
}

/// GET /api/v1/resources — list all unique resources across the registry
pub async fn resources_index(
    State(db): State<DbState>,
    Query(params): Query<ToolsQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let all_resources = db.list_resources()?;

    let mut items: Vec<serde_json::Value> = all_resources
        .into_iter()
        .map(|(resource, servers)| {
            serde_json::json!({
                "resource": resource,
                "server_count": servers.len(),
                "servers": servers,
            })
        })
        .collect();

    if let Some(ref q) = params.q {
        let q_lower = q.to_lowercase();
        items.retain(|item| {
            item["resource"]
                .as_str()
                .map(|r| r.to_lowercase().contains(&q_lower))
                .unwrap_or(false)
        });
    }

    let total = items.len();
    let limit = params.limit.unwrap_or(100).min(500);
    items.truncate(limit);

    Ok(Json(serde_json::json!({
        "resources": items,
        "total": total,
    })))
}

/// GET /api/v1/suggest — autocomplete server names by prefix
pub async fn suggest(
    State(db): State<DbState>,
    Query(params): Query<SuggestQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let prefix = params.q.unwrap_or_default();
    let limit = params.limit.unwrap_or(10).min(50);
    let db = db.lock().await;
    let suggestions = db.suggest(&prefix, limit)?;
    Ok(Json(serde_json::json!({
        "suggestions": suggestions,
        "total": suggestions.len(),
    })))
}

#[derive(Deserialize)]
pub struct SuggestQuery {
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

/// POST /api/v1/validate — validate a server entry without publishing
pub async fn validate_entry(
    Json(entry): Json<ServerEntry>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Required fields
    if entry.owner.is_empty() {
        errors.push("owner is required".into());
    }
    if entry.name.is_empty() {
        errors.push("name is required".into());
    }
    if entry.command.is_empty() {
        errors.push("command is required".into());
    }
    if entry.version.is_empty() {
        errors.push("version is required".into());
    } else {
        let parts: Vec<&str> = entry.version.split('.').collect();
        if parts.len() < 2 || parts.iter().any(|p| p.parse::<u64>().is_err()) {
            errors.push("version must be in semver format (e.g. 1.0.0)".into());
        }
    }

    // Transport validation
    let valid_transports = ["stdio", "sse", "streamable-http"];
    if !entry.transport.is_empty() && !valid_transports.contains(&entry.transport.as_str()) {
        errors.push(format!(
            "transport '{}' is not recognized (expected: {})",
            entry.transport,
            valid_transports.join(", ")
        ));
    }

    // Recommendations
    if entry.description.is_empty() {
        warnings.push("description is empty — a good description helps discovery".into());
    }
    if entry.license.is_empty() {
        warnings.push("license is empty — consider specifying one (MIT, Apache-2.0, etc.)".into());
    }
    if entry.tools.is_empty() && entry.resources.is_empty() && entry.prompts.is_empty() {
        warnings.push("no tools, resources, or prompts declared — consider adding capabilities".into());
    }
    if entry.repository.is_empty() {
        warnings.push("repository URL is empty — linking to source builds trust".into());
    }
    if entry.author.is_empty() {
        warnings.push("author is empty".into());
    }

    // Name format check
    if !entry.name.is_empty()
        && !entry
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        warnings.push("name contains non-standard characters — use lowercase alphanumeric, hyphens, or underscores".into());
    }

    // Env var hints validation
    for key in entry.env.keys() {
        if key.is_empty() {
            warnings.push("env contains an empty key".into());
        } else if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            warnings.push(format!("env key '{}' should use UPPER_SNAKE_CASE", key));
        }
    }

    // Homepage URL validation
    if !entry.homepage.is_empty() && !entry.homepage.starts_with("http://") && !entry.homepage.starts_with("https://") {
        warnings.push("homepage should be a valid HTTP(S) URL".into());
    }

    let valid = errors.is_empty();

    Ok(Json(serde_json::json!({
        "valid": valid,
        "errors": errors,
        "warnings": warnings,
        "server": format!("{}/{}", entry.owner, entry.name),
        "version": entry.version,
    })))
}

/// GET /api/v1/trending — top servers by downloads with optional filters
pub async fn trending(
    State(db): State<DbState>,
    Query(params): Query<TrendingQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let limit = params.limit.unwrap_or(15).min(100);
    let db = db.lock().await;
    let (mut servers, _) = db.list_servers(1, 1000)?;

    // Sort by downloads descending
    servers.sort_by(|a, b| b.downloads.cmp(&a.downloads));

    // Category filter
    if let Some(ref cat) = params.category {
        let cat_lower = cat.to_lowercase();
        servers.retain(|s| {
            crate::registry::seed::server_category(&s.owner, &s.name)
                .to_lowercase()
                .contains(&cat_lower)
        });
    }

    // Transport filter
    if let Some(ref transport) = params.transport {
        let t_lower = transport.to_lowercase();
        servers.retain(|s| s.transport.to_lowercase() == t_lower);
    }

    servers.truncate(limit);

    let items: Vec<serde_json::Value> = servers
        .iter()
        .enumerate()
        .map(|(i, s)| {
            serde_json::json!({
                "rank": i + 1,
                "owner": s.owner,
                "name": s.name,
                "full_name": s.full_name(),
                "description": s.description,
                "downloads": s.downloads,
                "category": crate::registry::seed::server_category(&s.owner, &s.name),
                "transport": s.transport,
                "tools_count": s.tools.len(),
                "version": s.version,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "trending": items,
        "total": items.len(),
    })))
}

#[derive(Deserialize)]
pub struct TrendingQuery {
    pub limit: Option<usize>,
    pub category: Option<String>,
    pub transport: Option<String>,
}

/// GET /api/v1/graph — tool-sharing graph between servers
pub async fn tool_graph(
    State(db): State<DbState>,
    Query(params): Query<GraphQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let min_shared = params.min_shared.unwrap_or(1);
    let limit = params.limit.unwrap_or(50).min(200);

    let db = db.lock().await;
    let (servers, _) = db.list_servers(1, 1000)?;

    // Build adjacency: pairs of servers that share tools
    let mut edges: Vec<serde_json::Value> = Vec::new();

    for i in 0..servers.len() {
        if edges.len() >= limit {
            break;
        }
        let tools_i: std::collections::HashSet<&str> =
            servers[i].tools.iter().map(|s| s.as_str()).collect();
        if tools_i.is_empty() {
            continue;
        }

        for j in (i + 1)..servers.len() {
            let tools_j: std::collections::HashSet<&str> =
                servers[j].tools.iter().map(|s| s.as_str()).collect();
            let shared: Vec<&str> = tools_i.intersection(&tools_j).copied().collect();

            if shared.len() >= min_shared {
                edges.push(serde_json::json!({
                    "server_a": servers[i].full_name(),
                    "server_b": servers[j].full_name(),
                    "shared_tools": shared,
                    "shared_count": shared.len(),
                }));
                if edges.len() >= limit {
                    break;
                }
            }
        }
    }

    // Sort by shared count descending
    edges.sort_by(|a, b| {
        b["shared_count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["shared_count"].as_u64().unwrap_or(0))
    });

    // Build node list
    let mut nodes: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for edge in &edges {
        if let Some(a) = edge["server_a"].as_str() {
            nodes.insert(a.to_string());
        }
        if let Some(b) = edge["server_b"].as_str() {
            nodes.insert(b.to_string());
        }
    }

    Ok(Json(serde_json::json!({
        "nodes": nodes,
        "edges": edges,
        "total_edges": edges.len(),
        "total_nodes": nodes.len(),
    })))
}

#[derive(Deserialize)]
pub struct GraphQuery {
    pub min_shared: Option<usize>,
    pub limit: Option<usize>,
}

/// PATCH /api/v1/servers/:owner/:name — partial update
pub async fn patch_server(
    State(db): State<DbState>,
    Path((owner, name)): Path<(String, String)>,
    Json(patch): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let mut entry = db
        .get_server(&owner, &name)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner}/{name}")))?;

    let mut changed = Vec::new();

    if let Some(desc) = patch.get("description").and_then(|v| v.as_str()) {
        entry.description = desc.to_string();
        changed.push("description");
    }
    if let Some(version) = patch.get("version").and_then(|v| v.as_str()) {
        // Validate semver
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() < 2 || parts.iter().any(|p| p.parse::<u64>().is_err()) {
            return Err(McpRegError::Validation("version must be in semver format".into()));
        }
        entry.version = version.to_string();
        changed.push("version");
    }
    if let Some(author) = patch.get("author").and_then(|v| v.as_str()) {
        entry.author = author.to_string();
        changed.push("author");
    }
    if let Some(license) = patch.get("license").and_then(|v| v.as_str()) {
        entry.license = license.to_string();
        changed.push("license");
    }
    if let Some(repository) = patch.get("repository").and_then(|v| v.as_str()) {
        entry.repository = repository.to_string();
        changed.push("repository");
    }
    if let Some(tools) = patch.get("tools").and_then(|v| v.as_array()) {
        entry.tools = tools
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        changed.push("tools");
    }
    if let Some(resources) = patch.get("resources").and_then(|v| v.as_array()) {
        entry.resources = resources
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        changed.push("resources");
    }
    if let Some(prompts) = patch.get("prompts").and_then(|v| v.as_array()) {
        entry.prompts = prompts
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        changed.push("prompts");
    }
    if let Some(tags) = patch.get("tags").and_then(|v| v.as_array()) {
        entry.tags = tags
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        changed.push("tags");
    }
    if let Some(transport) = patch.get("transport").and_then(|v| v.as_str()) {
        let valid_transports = ["stdio", "sse", "streamable-http"];
        if !valid_transports.contains(&transport) {
            return Err(McpRegError::Validation(format!(
                "transport '{}' is not recognized",
                transport
            )));
        }
        entry.transport = transport.to_string();
        changed.push("transport");
    }
    if let Some(env_obj) = patch.get("env").and_then(|v| v.as_object()) {
        let mut env_map = std::collections::HashMap::new();
        for (k, v) in env_obj {
            if let Some(val) = v.as_str() {
                env_map.insert(k.clone(), val.to_string());
            }
        }
        entry.env = env_map;
        changed.push("env");
    }
    if let Some(homepage) = patch.get("homepage").and_then(|v| v.as_str()) {
        entry.homepage = homepage.to_string();
        changed.push("homepage");
    }
    if let Some(deprecated) = patch.get("deprecated").and_then(|v| v.as_bool()) {
        entry.deprecated = deprecated;
        changed.push("deprecated");
    }
    if let Some(deprecated_by) = patch.get("deprecated_by").and_then(|v| v.as_str()) {
        entry.deprecated_by = if deprecated_by.is_empty() { None } else { Some(deprecated_by.to_string()) };
        changed.push("deprecated_by");
    }

    if changed.is_empty() {
        return Err(McpRegError::Validation("no valid fields to update".into()));
    }

    db.upsert_server(&entry)?;

    Ok(Json(serde_json::json!({
        "success": true,
        "server": format!("{owner}/{name}"),
        "updated_fields": changed,
    })))
}

/// GET /api/v1/openapi — API documentation / endpoint listing
pub async fn openapi() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "openapi": "3.0.0",
        "info": {
            "title": "mcpreg API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Open source registry and marketplace for MCP (Model Context Protocol) servers",
        },
        "paths": {
            "/health": {
                "get": { "summary": "Health check", "tags": ["System"] }
            },
            "/api/v1/version": {
                "get": { "summary": "Server version info", "tags": ["System"] }
            },
            "/api/v1/openapi": {
                "get": { "summary": "API documentation (this endpoint)", "tags": ["System"] }
            },
            "/api/v1/search": {
                "get": {
                    "summary": "Search for MCP servers",
                    "tags": ["Servers"],
                    "parameters": [
                        {"name": "q", "in": "query", "description": "Search query (multi-word AND matching)"},
                        {"name": "category", "in": "query", "description": "Filter by category"},
                        {"name": "sort", "in": "query", "description": "Sort order: downloads, name, updated"},
                        {"name": "limit", "in": "query", "description": "Max results"},
                        {"name": "min_downloads", "in": "query", "description": "Minimum download count"},
                        {"name": "tool", "in": "query", "description": "Filter by tool name"},
                        {"name": "transport", "in": "query", "description": "Filter by transport type"},
                        {"name": "author", "in": "query", "description": "Filter by author"},
                        {"name": "owner", "in": "query", "description": "Filter by owner"},
                        {"name": "tag", "in": "query", "description": "Filter by tag"},
                        {"name": "min_tools", "in": "query", "description": "Minimum number of tools"},
                        {"name": "has_prompts", "in": "query", "description": "Only servers with prompts (true)"},
                        {"name": "resource", "in": "query", "description": "Filter by resource type"},
                    ]
                }
            },
            "/api/v1/servers": {
                "get": {
                    "summary": "List servers (paginated)",
                    "tags": ["Servers"],
                    "parameters": [
                        {"name": "page", "in": "query", "description": "Page number (default: 1)"},
                        {"name": "per_page", "in": "query", "description": "Items per page (max 100)"},
                    ]
                }
            },
            "/api/v1/servers/{owner}/{name}": {
                "get": { "summary": "Get server details (increments download count)", "tags": ["Servers"] },
                "delete": { "summary": "Delete a server", "tags": ["Servers"] },
                "patch": { "summary": "Partial update a server", "tags": ["Servers"] },
            },
            "/api/v1/servers/{owner}/{name}/download": {
                "post": { "summary": "Track a download without fetching details", "tags": ["Servers"] }
            },
            "/api/v1/servers/{owner}/{name}/versions": {
                "get": { "summary": "Version history for a server", "tags": ["Servers"] }
            },
            "/api/v1/servers/{owner}/{name}/similar": {
                "get": { "summary": "Find similar servers", "tags": ["Discovery"] }
            },
            "/api/v1/servers/{owner}/{name}/diff": {
                "get": { "summary": "Diff current state vs previous version", "tags": ["Servers"] }
            },
            "/api/v1/servers/{owner}/{name}/dependents": {
                "get": { "summary": "Find servers that share tools with this one", "tags": ["Discovery"] }
            },
            "/api/v1/servers/batch": {
                "post": { "summary": "Fetch multiple servers by owner/name pairs", "tags": ["Servers"] }
            },
            "/api/v1/publish": {
                "post": { "summary": "Publish a server to the registry", "tags": ["Publishing"] }
            },
            "/api/v1/validate": {
                "post": { "summary": "Validate a server entry without publishing", "tags": ["Publishing"] }
            },
            "/api/v1/stats": {
                "get": { "summary": "Aggregate registry statistics", "tags": ["Analytics"] }
            },
            "/api/v1/tools": {
                "get": { "summary": "List all unique tools across servers", "tags": ["Discovery"] }
            },
            "/api/v1/prompts": {
                "get": { "summary": "List all unique prompts across servers", "tags": ["Discovery"] }
            },
            "/api/v1/tags": {
                "get": { "summary": "List all unique tags across servers", "tags": ["Discovery"] }
            },
            "/api/v1/resources": {
                "get": { "summary": "List all unique resources across servers", "tags": ["Discovery"] }
            },
            "/api/v1/categories": {
                "get": { "summary": "List servers grouped by category", "tags": ["Discovery"] }
            },
            "/api/v1/trending": {
                "get": { "summary": "Top servers by downloads", "tags": ["Analytics"] }
            },
            "/api/v1/graph": {
                "get": { "summary": "Tool-sharing graph between servers", "tags": ["Discovery"] }
            },
            "/api/v1/suggest": {
                "get": {
                    "summary": "Autocomplete server names by prefix",
                    "tags": ["Discovery"],
                    "parameters": [
                        {"name": "q", "in": "query", "description": "Prefix to match"},
                        {"name": "limit", "in": "query", "description": "Max suggestions (default 10, max 50)"},
                    ]
                }
            },
            "/api/v1/random": {
                "get": {
                    "summary": "Discover a random MCP server",
                    "tags": ["Discovery"],
                    "parameters": [
                        {"name": "category", "in": "query", "description": "Optional category filter"},
                    ]
                }
            },
            "/api/v1/servers/{owner}/{name}/config": {
                "get": { "summary": "Get claude_desktop_config.json snippet for a server", "tags": ["Servers"] }
            },
            "/api/v1/servers/batch/delete": {
                "delete": { "summary": "Bulk delete servers by owner/name pairs", "tags": ["Servers"] }
            },
            "/api/v1/export": {
                "get": { "summary": "Full registry dump as JSON", "tags": ["System"] }
            },
            "/api/v1/owners": {
                "get": { "summary": "List all owners with server counts", "tags": ["Discovery"] }
            },
            "/api/v1/search/any": {
                "get": {
                    "summary": "OR-based search with pipe-separated terms (e.g. q=postgres|sqlite|redis)",
                    "tags": ["Servers"],
                    "parameters": [
                        {"name": "q", "in": "query", "description": "Pipe-separated search terms (OR matching)"},
                    ]
                }
            },
            "/api/v1/changelog": {
                "get": {
                    "summary": "Recent version publications across the registry",
                    "tags": ["Analytics"],
                    "parameters": [
                        {"name": "limit", "in": "query", "description": "Max entries (default 25, max 100)"},
                    ]
                }
            },
            "/api/v1/recently-updated": {
                "get": {
                    "summary": "Servers ordered by most recently updated",
                    "tags": ["Discovery"],
                    "parameters": [
                        {"name": "limit", "in": "query", "description": "Max results (default 20, max 100)"},
                    ]
                }
            },
            "/api/v1/servers/{owner}/{name}/bundle": {
                "get": {
                    "summary": "Recommend complementary servers to pair with the given server",
                    "tags": ["Discovery"],
                    "parameters": [
                        {"name": "owner", "in": "path", "required": true},
                        {"name": "name", "in": "path", "required": true},
                        {"name": "limit", "in": "query", "description": "Max results (default 5, max 20)"},
                    ]
                }
            },
            "/api/v1/servers/{owner}/{name}/score": {
                "get": {
                    "summary": "Quality score for a server based on metadata completeness",
                    "tags": ["Inspection"],
                    "parameters": [
                        {"name": "owner", "in": "path", "required": true},
                        {"name": "name", "in": "path", "required": true},
                    ]
                }
            },
        }
    }))
}

/// GET /api/v1/servers/:owner/:name/dependents — find servers that share tools
pub async fn dependents(
    State(db): State<DbState>,
    Path((owner, name)): Path<(String, String)>,
    Query(params): Query<SimilarQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let limit = params.limit.unwrap_or(10).min(50);
    let db = db.lock().await;
    let target = db
        .get_server(&owner, &name)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner}/{name}")))?;

    if target.tools.is_empty() {
        return Ok(Json(serde_json::json!({
            "server": format!("{owner}/{name}"),
            "dependents": [],
            "total": 0,
            "message": "Server has no tools declared",
        })));
    }

    let target_tools: std::collections::HashSet<&str> =
        target.tools.iter().map(|s| s.as_str()).collect();

    let (all_servers, _) = db.list_servers(1, 1000)?;

    let mut dependents: Vec<serde_json::Value> = all_servers
        .iter()
        .filter(|s| !(s.owner == owner && s.name == name))
        .filter_map(|s| {
            let s_tools: std::collections::HashSet<&str> =
                s.tools.iter().map(|t| t.as_str()).collect();
            let shared: Vec<String> = target_tools
                .intersection(&s_tools)
                .map(|t| t.to_string())
                .collect();
            if shared.is_empty() {
                None
            } else {
                Some(serde_json::json!({
                    "owner": s.owner,
                    "name": s.name,
                    "full_name": s.full_name(),
                    "shared_tools": shared,
                    "shared_count": shared.len(),
                    "downloads": s.downloads,
                }))
            }
        })
        .collect();

    dependents.sort_by(|a, b| {
        b["shared_count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["shared_count"].as_u64().unwrap_or(0))
    });
    dependents.truncate(limit);

    let total = dependents.len();
    Ok(Json(serde_json::json!({
        "server": format!("{owner}/{name}"),
        "dependents": dependents,
        "total": total,
    })))
}

/// GET /api/v1/random — discover a random MCP server
pub async fn random_server(
    State(db): State<DbState>,
    Query(params): Query<RandomQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let server = db.random_server(params.category.as_deref())?;

    match server {
        Some(s) => Ok(Json(serde_json::json!({
            "server": {
                "owner": s.owner,
                "name": s.name,
                "full_name": s.full_name(),
                "version": s.version,
                "description": s.description,
                "author": s.author,
                "license": s.license,
                "repository": s.repository,
                "command": s.command,
                "args": s.args,
                "transport": s.transport,
                "tools": s.tools,
                "resources": s.resources,
                "prompts": s.prompts,
                "tags": s.tags,
                "downloads": s.downloads,
                "category": crate::registry::seed::server_category(&s.owner, &s.name),
            }
        }))),
        None => {
            let msg = if params.category.is_some() {
                "No servers found in this category"
            } else {
                "No servers in registry"
            };
            Err(McpRegError::NotFound(msg.into()))
        }
    }
}

#[derive(Deserialize)]
pub struct RandomQuery {
    pub category: Option<String>,
}

#[derive(Deserialize)]
pub struct ToolIndexQuery {
    pub q: Option<String>,
    pub limit: Option<usize>,
}

/// GET /api/v1/servers/:owner/:name/config — return claude_desktop_config.json snippet
pub async fn server_config_snippet(
    State(db): State<DbState>,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let entry = db
        .get_server(&owner, &name)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner}/{name}")))?;

    let key = format!("{}-{}", owner, entry.name);

    // Build the mcpServers config snippet
    let mut server_config = serde_json::json!({
        "command": entry.command,
        "args": entry.args,
    });

    // Only add transport if not the default (stdio)
    if entry.transport != "stdio" {
        server_config["transport"] = serde_json::json!(entry.transport);
    }

    // Include env vars if any are declared
    if !entry.env.is_empty() {
        server_config["env"] = serde_json::json!(entry.env);
    }

    let snippet = serde_json::json!({
        "mcpServers": {
            key.clone(): server_config
        }
    });

    Ok(Json(serde_json::json!({
        "server": entry.full_name(),
        "version": entry.version,
        "config_key": key,
        "claude_desktop_config": snippet,
        "instructions": format!(
            "Add this to your claude_desktop_config.json under \"mcpServers\":\n\n\"{}\":\n  command: {}\n  args: {:?}",
            key, entry.command, entry.args
        ),
    })))
}

/// DELETE /api/v1/servers/batch — bulk delete servers
pub async fn batch_delete_servers(
    State(db): State<DbState>,
    Json(body): Json<BatchRequest>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    if body.servers.is_empty() {
        return Ok(Json(serde_json::json!({"deleted": 0, "not_found": [], "total_requested": 0})));
    }
    if body.servers.len() > 50 {
        return Err(McpRegError::Validation("Maximum 50 servers per batch request".into()));
    }

    let mut refs = Vec::new();
    let mut invalid = Vec::new();
    for ref_str in &body.servers {
        let parts: Vec<&str> = ref_str.splitn(2, '/').collect();
        if parts.len() == 2 {
            refs.push((parts[0].to_string(), parts[1].to_string()));
        } else {
            invalid.push(ref_str.clone());
        }
    }

    let db = db.lock().await;
    let deleted = db.bulk_delete(&refs)?;
    let not_found_count = refs.len() - deleted;

    Ok(Json(serde_json::json!({
        "deleted": deleted,
        "total_requested": body.servers.len(),
        "invalid_refs": invalid,
        "not_found_count": not_found_count,
    })))
}

/// GET /api/v1/export — full registry dump as JSON
pub async fn export_registry(
    State(db): State<DbState>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let servers = db.export_all()?;
    let total = servers.len();
    Ok(Json(serde_json::json!({
        "servers": servers,
        "total": total,
        "exported_at": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        "version": env!("CARGO_PKG_VERSION"),
    })))
}

/// GET /api/v1/owners — list all owners with server counts
pub async fn list_owners(
    State(db): State<DbState>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let owners = db.list_owners()?;
    let items: Vec<serde_json::Value> = owners
        .iter()
        .map(|(owner, count)| {
            serde_json::json!({
                "owner": owner,
                "server_count": count,
            })
        })
        .collect();
    let total = items.len();
    Ok(Json(serde_json::json!({
        "owners": items,
        "total": total,
    })))
}

/// GET /api/v1/search/any — OR-based search (pipe-separated terms)
pub async fn search_any(
    State(db): State<DbState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, McpRegError> {
    let query = params.q.unwrap_or_default();
    let db = db.lock().await;
    let servers = db.search_any(&query)?;
    let total = servers.len();
    Ok(Json(SearchResponse { servers, total, suggestions: None }))
}

/// GET /api/v1/changelog — recent version publications across the registry
pub async fn changelog(
    State(db): State<DbState>,
    Query(params): Query<ChangelogQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let limit = params.limit.unwrap_or(25).min(100);
    let db = db.lock().await;
    let versions = db.recent_versions(limit)?;
    let items: Vec<serde_json::Value> = versions
        .iter()
        .map(|(owner, name, version, published_at)| {
            serde_json::json!({
                "server": format!("{owner}/{name}"),
                "version": version,
                "published_at": published_at,
            })
        })
        .collect();
    let total = items.len();
    Ok(Json(serde_json::json!({
        "changelog": items,
        "total": total,
    })))
}

#[derive(Deserialize)]
pub struct ChangelogQuery {
    pub limit: Option<usize>,
}

/// GET /api/v1/recently-updated — servers ordered by update time
pub async fn recently_updated(
    State(db): State<DbState>,
    Query(params): Query<RecentlyUpdatedQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let limit = params.limit.unwrap_or(20).min(100);
    let db = db.lock().await;
    let servers = db.recently_updated(limit)?;
    let items: Vec<serde_json::Value> = servers
        .iter()
        .map(|s| {
            serde_json::json!({
                "owner": s.owner,
                "name": s.name,
                "full_name": s.full_name(),
                "version": s.version,
                "description": s.description,
                "updated_at": s.updated_at,
                "downloads": s.downloads,
            })
        })
        .collect();
    let total = items.len();
    Ok(Json(serde_json::json!({
        "servers": items,
        "total": total,
    })))
}

#[derive(Deserialize)]
pub struct RecentlyUpdatedQuery {
    pub limit: Option<usize>,
}

/// Bulk import servers (POST /api/v1/import)
pub async fn bulk_import(
    State(db): State<DbState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let servers = payload
        .get("servers")
        .and_then(|v| v.as_array())
        .ok_or_else(|| McpRegError::Validation("body must contain a 'servers' array".into()))?;

    if servers.len() > 500 {
        return Err(McpRegError::Validation("maximum 500 servers per import".into()));
    }

    let mut imported = 0usize;
    let mut errors = Vec::<serde_json::Value>::new();
    let db = db.lock().await;

    for (i, raw) in servers.iter().enumerate() {
        match serde_json::from_value::<ServerEntry>(raw.clone()) {
            Ok(entry) => {
                if entry.owner.is_empty() || entry.name.is_empty() {
                    errors.push(serde_json::json!({
                        "index": i,
                        "error": "owner and name are required"
                    }));
                    continue;
                }
                match db.upsert_server(&entry) {
                    Ok(_) => imported += 1,
                    Err(e) => errors.push(serde_json::json!({
                        "index": i,
                        "server": entry.full_name(),
                        "error": e.to_string()
                    })),
                }
            }
            Err(e) => {
                errors.push(serde_json::json!({
                    "index": i,
                    "error": format!("invalid server entry: {e}")
                }));
            }
        }
    }

    Ok(Json(serde_json::json!({
        "imported": imported,
        "errors": errors,
        "total_submitted": servers.len(),
    })))
}

/// Compare two servers side-by-side (GET /api/v1/compare/:owner_a/:name_a/:owner_b/:name_b)
pub async fn compare_servers(
    State(db): State<DbState>,
    Path((owner_a, name_a, owner_b, name_b)): Path<(String, String, String, String)>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let a = db.get_server(&owner_a, &name_a)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner_a}/{name_a}")))?;
    let b = db.get_server(&owner_b, &name_b)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner_b}/{name_b}")))?;

    let tools_a: std::collections::HashSet<&str> = a.tools.iter().map(|s| s.as_str()).collect();
    let tools_b: std::collections::HashSet<&str> = b.tools.iter().map(|s| s.as_str()).collect();
    let shared_tools: Vec<&str> = tools_a.intersection(&tools_b).copied().collect();
    let only_a_tools: Vec<&str> = tools_a.difference(&tools_b).copied().collect();
    let only_b_tools: Vec<&str> = tools_b.difference(&tools_a).copied().collect();

    let res_a: std::collections::HashSet<&str> = a.resources.iter().map(|s| s.as_str()).collect();
    let res_b: std::collections::HashSet<&str> = b.resources.iter().map(|s| s.as_str()).collect();
    let shared_resources: Vec<&str> = res_a.intersection(&res_b).copied().collect();

    let prompts_a: std::collections::HashSet<&str> = a.prompts.iter().map(|s| s.as_str()).collect();
    let prompts_b: std::collections::HashSet<&str> = b.prompts.iter().map(|s| s.as_str()).collect();
    let shared_prompts: Vec<&str> = prompts_a.intersection(&prompts_b).copied().collect();

    Ok(Json(serde_json::json!({
        "server_a": {
            "full_name": a.full_name(),
            "version": a.version,
            "description": a.description,
            "transport": a.transport,
            "tools": a.tools,
            "resources": a.resources,
            "prompts": a.prompts,
            "downloads": a.downloads,
            "deprecated": a.deprecated,
        },
        "server_b": {
            "full_name": b.full_name(),
            "version": b.version,
            "description": b.description,
            "transport": b.transport,
            "tools": b.tools,
            "resources": b.resources,
            "prompts": b.prompts,
            "downloads": b.downloads,
            "deprecated": b.deprecated,
        },
        "comparison": {
            "shared_tools": shared_tools,
            "only_a_tools": only_a_tools,
            "only_b_tools": only_b_tools,
            "shared_resources": shared_resources,
            "shared_prompts": shared_prompts,
            "same_transport": a.transport == b.transport,
        }
    })))
}

/// List all deprecated servers (GET /api/v1/deprecated)
pub async fn list_deprecated(
    State(db): State<DbState>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let all = db.list_all()?;
    let deprecated: Vec<serde_json::Value> = all
        .into_iter()
        .filter(|s| s.deprecated)
        .map(|s| {
            serde_json::json!({
                "full_name": s.full_name(),
                "version": s.version,
                "description": s.description,
                "deprecated_by": s.deprecated_by,
                "downloads": s.downloads,
            })
        })
        .collect();
    let total = deprecated.len();
    Ok(Json(serde_json::json!({
        "servers": deprecated,
        "total": total,
    })))
}

/// Popular tools ranked by aggregate downloads of servers that use them (GET /api/v1/popular-tools)
pub async fn popular_tools(
    State(db): State<DbState>,
    Query(params): Query<ToolIndexQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let all = db.list_all()?;

    // Aggregate downloads per tool
    let mut tool_scores: std::collections::HashMap<String, (i64, Vec<String>)> =
        std::collections::HashMap::new();

    for server in &all {
        for tool in &server.tools {
            let entry = tool_scores
                .entry(tool.clone())
                .or_insert_with(|| (0, Vec::new()));
            entry.0 += server.downloads;
            entry.1.push(server.full_name());
        }
    }

    let mut ranked: Vec<(String, i64, Vec<String>)> = tool_scores
        .into_iter()
        .map(|(tool, (downloads, servers))| (tool, downloads, servers))
        .collect();

    // Filter by query if provided
    if let Some(ref q) = params.q {
        let q_lower = q.to_lowercase();
        ranked.retain(|(tool, _, _)| tool.to_lowercase().contains(&q_lower));
    }

    ranked.sort_by(|a, b| b.1.cmp(&a.1));

    let limit = params.limit.unwrap_or(50).min(200);
    ranked.truncate(limit);

    let items: Vec<serde_json::Value> = ranked
        .iter()
        .enumerate()
        .map(|(i, (tool, downloads, servers))| {
            serde_json::json!({
                "rank": i + 1,
                "tool": tool,
                "aggregate_downloads": downloads,
                "server_count": servers.len(),
                "servers": servers,
            })
        })
        .collect();

    let total = items.len();
    Ok(Json(serde_json::json!({
        "tools": items,
        "total": total,
    })))
}

/// Server compatibility check (GET /api/v1/compatibility/:owner_a/:name_a/:owner_b/:name_b)
pub async fn compatibility(
    State(db): State<DbState>,
    Path((owner_a, name_a, owner_b, name_b)): Path<(String, String, String, String)>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let a = db
        .get_server(&owner_a, &name_a)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner_a}/{name_a} not found")))?;
    let b = db
        .get_server(&owner_b, &name_b)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner_b}/{name_b} not found")))?;

    // Check for tool name conflicts (same tool name)
    let tools_a: std::collections::HashSet<&str> =
        a.tools.iter().map(|t| t.as_str()).collect();
    let tools_b: std::collections::HashSet<&str> =
        b.tools.iter().map(|t| t.as_str()).collect();
    let conflicting_tools: Vec<&str> = tools_a.intersection(&tools_b).copied().collect();

    // Check env key overlaps
    let env_a: std::collections::HashSet<&str> =
        a.env.keys().map(|k| k.as_str()).collect();
    let env_b: std::collections::HashSet<&str> =
        b.env.keys().map(|k| k.as_str()).collect();
    let shared_env: Vec<&str> = env_a.intersection(&env_b).copied().collect();

    // Resource conflicts
    let res_a: std::collections::HashSet<&str> =
        a.resources.iter().map(|r| r.as_str()).collect();
    let res_b: std::collections::HashSet<&str> =
        b.resources.iter().map(|r| r.as_str()).collect();
    let shared_resources: Vec<&str> = res_a.intersection(&res_b).copied().collect();

    // Compute compatibility score (0-100)
    let mut score = 100i32;
    let mut issues: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    if !conflicting_tools.is_empty() {
        score -= (conflicting_tools.len() as i32 * 10).min(40);
        issues.push(format!(
            "{} conflicting tool name(s): {}",
            conflicting_tools.len(),
            conflicting_tools.join(", ")
        ));
    }

    if !shared_env.is_empty() {
        score -= (shared_env.len() as i32 * 5).min(20);
        issues.push(format!(
            "{} shared env variable(s): {} — may need different values",
            shared_env.len(),
            shared_env.join(", ")
        ));
    }

    if a.transport != b.transport {
        notes.push(format!(
            "Different transports: {} vs {} — both supported by most clients",
            a.transport, b.transport
        ));
    } else {
        notes.push(format!("Same transport: {}", a.transport));
    }

    if !shared_resources.is_empty() {
        notes.push(format!(
            "Shared resource types: {} — may complement each other",
            shared_resources.join(", ")
        ));
    }

    let score = score.max(0);
    let compatible = score >= 70;

    Ok(Json(serde_json::json!({
        "server_a": a.full_name(),
        "server_b": b.full_name(),
        "compatible": compatible,
        "score": score,
        "issues": issues,
        "notes": notes,
        "details": {
            "conflicting_tools": conflicting_tools,
            "shared_env_keys": shared_env,
            "shared_resources": shared_resources,
            "transport_match": a.transport == b.transport,
            "combined_tool_count": tools_a.len() + tools_b.len() - conflicting_tools.len(),
        }
    })))
}

/// Recommend server bundles based on a seed server (servers commonly paired together).
pub async fn recommend_bundle(
    State(db): State<DbState>,
    Path((owner, name)): Path<(String, String)>,
    Query(params): Query<LimitQuery>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let seed = db.get_server(&owner, &name)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner}/{name}")))?;

    let all = db.list_all()?;
    let limit = params.limit.unwrap_or(5).min(20);

    // Score each server based on tool overlap, category match, and complementary resources
    let seed_tools: std::collections::HashSet<String> =
        seed.tools.iter().map(|t| t.to_lowercase()).collect();
    let seed_resources: std::collections::HashSet<String> =
        seed.resources.iter().map(|r| r.to_lowercase()).collect();
    let seed_cat = crate::registry::seed::server_category(&seed.owner, &seed.name).to_lowercase();

    let mut scored: Vec<(crate::api::types::ServerEntry, i64)> = all
        .into_iter()
        .filter(|s| s.full_name() != seed.full_name() && !s.deprecated)
        .map(|s| {
            let s_tools: std::collections::HashSet<String> =
                s.tools.iter().map(|t| t.to_lowercase()).collect();
            let s_resources: std::collections::HashSet<String> =
                s.resources.iter().map(|r| r.to_lowercase()).collect();
            let s_cat = crate::registry::seed::server_category(&s.owner, &s.name).to_lowercase();

            let mut score: i64 = 0;

            // Complementary tools (tools the seed doesn't have) = good
            let unique_tools = s_tools.difference(&seed_tools).count();
            score += unique_tools as i64 * 10;

            // Some tool overlap = related but not redundant
            let shared_tools = s_tools.intersection(&seed_tools).count();
            if shared_tools > 0 && shared_tools <= 2 {
                score += 5;
            }
            // Too much overlap = redundant
            if shared_tools > s_tools.len() / 2 && s_tools.len() > 2 {
                score -= 20;
            }

            // Same category = related
            if s_cat == seed_cat {
                score += 15;
            }

            // Complementary resources
            let new_resources = s_resources.difference(&seed_resources).count();
            score += new_resources as i64 * 5;

            // Popularity bonus (log scale)
            if s.downloads > 0 {
                score += (s.downloads as f64).log10() as i64;
            }

            (s, score)
        })
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.truncate(limit);

    let bundle: Vec<serde_json::Value> = scored
        .iter()
        .enumerate()
        .map(|(i, (s, score))| {
            serde_json::json!({
                "rank": i + 1,
                "full_name": s.full_name(),
                "description": s.description,
                "tools": s.tools,
                "score": score,
                "reason": if *score > 30 { "highly complementary" }
                    else if *score > 15 { "good addition" }
                    else { "related" },
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "seed": seed.full_name(),
        "bundle": bundle,
        "total": bundle.len(),
    })))
}

/// Server health score — a quality metric based on completeness of metadata.
pub async fn server_score(
    State(db): State<DbState>,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, McpRegError> {
    let db = db.lock().await;
    let server = db.get_server(&owner, &name)?
        .ok_or_else(|| McpRegError::NotFound(format!("{owner}/{name}")))?;

    let mut score: u32 = 0;
    let mut max_score: u32 = 0;
    let mut checks: Vec<serde_json::Value> = Vec::new();

    // Required fields (always present if published, but check quality)
    let check = |name: &str, pass: bool, points: u32, checks: &mut Vec<serde_json::Value>, score: &mut u32, max: &mut u32| {
        *max += points;
        if pass {
            *score += points;
        }
        checks.push(serde_json::json!({
            "check": name,
            "pass": pass,
            "points": if pass { points } else { 0 },
            "max_points": points,
        }));
    };

    check("has_description", !server.description.is_empty(), 15, &mut checks, &mut score, &mut max_score);
    check("has_author", !server.author.is_empty(), 10, &mut checks, &mut score, &mut max_score);
    check("has_license", !server.license.is_empty(), 10, &mut checks, &mut score, &mut max_score);
    check("has_repository", !server.repository.is_empty(), 10, &mut checks, &mut score, &mut max_score);
    check("has_homepage", !server.homepage.is_empty(), 5, &mut checks, &mut score, &mut max_score);
    check("has_tools", !server.tools.is_empty(), 15, &mut checks, &mut score, &mut max_score);
    check("has_resources", !server.resources.is_empty(), 5, &mut checks, &mut score, &mut max_score);
    check("has_prompts", !server.prompts.is_empty(), 5, &mut checks, &mut score, &mut max_score);
    check("has_tags", !server.tags.is_empty(), 5, &mut checks, &mut score, &mut max_score);
    check("multiple_tools", server.tools.len() >= 2, 5, &mut checks, &mut score, &mut max_score);
    check("not_deprecated", !server.deprecated, 10, &mut checks, &mut score, &mut max_score);
    check("has_downloads", server.downloads > 0, 5, &mut checks, &mut score, &mut max_score);

    let percentage = if max_score > 0 {
        (score as f64 / max_score as f64 * 100.0).round() as u32
    } else {
        0
    };

    let grade = match percentage {
        90..=100 => "A",
        80..=89 => "B",
        70..=79 => "C",
        60..=69 => "D",
        _ => "F",
    };

    Ok(Json(serde_json::json!({
        "server": server.full_name(),
        "score": score,
        "max_score": max_score,
        "percentage": percentage,
        "grade": grade,
        "checks": checks,
    })))
}
