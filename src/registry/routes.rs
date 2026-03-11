use crate::api::types::{PaginatedResponse, PublishResponse, SearchResponse, ServerEntry};
#[allow(unused_imports)]
use crate::registry::db::Database;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
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
) -> Result<Json<SearchResponse>, StatusCode> {
    let query = params.q.unwrap_or_default();
    let db = db.lock().await;
    match db.search(&query) {
        Ok(servers) => {
            let total = servers.len();
            Ok(Json(SearchResponse { servers, total }))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn get_server(
    State(db): State<DbState>,
    Path((owner, name)): Path<(String, String)>,
) -> Result<Json<ServerEntry>, StatusCode> {
    let db = db.lock().await;
    match db.get_server(&owner, &name) {
        Ok(Some(entry)) => Ok(Json(entry)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn publish(
    State(db): State<DbState>,
    Json(entry): Json<ServerEntry>,
) -> Result<Json<PublishResponse>, StatusCode> {
    // Note: In production, validate the Authorization header / API key here
    let db = db.lock().await;
    match db.upsert_server(&entry) {
        Ok(_) => Ok(Json(PublishResponse {
            success: true,
            message: format!("Published {}/{} v{}", entry.owner, entry.name, entry.version),
        })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn list_servers(
    State(db): State<DbState>,
    Query(params): Query<PaginationQuery>,
) -> Result<Json<PaginatedResponse>, StatusCode> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let db = db.lock().await;
    match db.list_servers(page, per_page) {
        Ok((servers, total)) => Ok(Json(PaginatedResponse {
            servers,
            page,
            per_page,
            total,
        })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn health() -> &'static str {
    "ok"
}

/// GET /api/v1/stats — aggregate registry statistics
pub async fn stats(
    State(db): State<DbState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let db = db.lock().await;
    match db.stats() {
        Ok(s) => Ok(Json(serde_json::json!({
            "total_servers": s.total_servers,
            "total_downloads": s.total_downloads,
            "unique_owners": s.unique_owners,
            "avg_tools": s.avg_tools,
            "top_servers": s.top_servers.iter().map(|(n, d)| serde_json::json!({"name": n, "downloads": d})).collect::<Vec<_>>(),
            "transports": s.transport_counts.iter().map(|(t, c)| serde_json::json!({"transport": t, "count": c})).collect::<Vec<_>>(),
        }))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
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
) -> Result<Json<serde_json::Value>, StatusCode> {
    use crate::registry::seed::server_category;
    use std::collections::BTreeMap;

    let db = db.lock().await;
    let (servers, _) = db.list_servers(1, 1000).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
}
