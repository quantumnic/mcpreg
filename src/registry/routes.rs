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
    let servers = db.search(&query)?;
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

pub async fn health() -> &'static str {
    "ok"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint() {
        let result = health().await;
        assert_eq!(result, "ok");
    }

    #[tokio::test]
    async fn test_version_endpoint() {
        let result = version().await;
        let v = result.0;
        assert_eq!(v["name"], "mcpreg");
        assert!(!v["version"].as_str().unwrap().is_empty());
    }
}
