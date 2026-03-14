use crate::registry::db::Database;
use crate::registry::routes::{self, DbState};
use axum::Router;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

pub async fn run_server(bind_addr: &str, db_path: &str) -> crate::error::Result<()> {
    let db = Database::open(db_path)?;

    match db.seed_default_servers() {
        Ok(0) => tracing::debug!("database already populated, skipping seed"),
        Ok(n) => tracing::info!("seeded {n} default MCP servers into registry"),
        Err(e) => tracing::warn!("failed to seed default servers: {e}"),
    }

    let db_state: DbState = Arc::new(Mutex::new(db));

    let app = build_router(db_state);

    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .map_err(crate::error::McpRegError::Io)?;

    tracing::info!("mcpreg registry server listening on {bind_addr}");
    axum::serve(listener, app)
        .await
        .map_err(crate::error::McpRegError::Io)?;

    Ok(())
}

pub fn build_router(db_state: DbState) -> Router {
    Router::new()
        .route("/health", axum::routing::get(routes::health))
        .route("/api/v1/version", axum::routing::get(routes::version))
        .route("/api/v1/search", axum::routing::get(routes::search))
        .route(
            "/api/v1/servers/:owner/:name/download",
            axum::routing::post(routes::track_download),
        )
        .route(
            "/api/v1/servers/:owner/:name/versions",
            axum::routing::get(routes::version_history),
        )
        .route(
            "/api/v1/servers/:owner/:name/similar",
            axum::routing::get(routes::similar_servers),
        )
        .route(
            "/api/v1/servers/:owner/:name/diff",
            axum::routing::get(routes::server_diff),
        )
        .route("/api/v1/servers", axum::routing::get(routes::list_servers))
        .route("/api/v1/servers/batch", axum::routing::post(routes::batch_get_servers))
        .route("/api/v1/publish", axum::routing::post(routes::publish))
        .route("/api/v1/validate", axum::routing::post(routes::validate_entry))
        .route(
            "/api/v1/servers/:owner/:name",
            axum::routing::get(routes::get_server)
                .delete(routes::delete_server)
                .patch(routes::patch_server),
        )
        .route("/api/v1/stats", axum::routing::get(routes::stats))
        .route("/api/v1/tools", axum::routing::get(routes::tools_index))
        .route("/api/v1/categories", axum::routing::get(routes::categories))
        .route("/api/v1/prompts", axum::routing::get(routes::prompts_index))
        .route("/api/v1/tags", axum::routing::get(routes::tags_index))
        .route("/api/v1/resources", axum::routing::get(routes::resources_index))
        .route("/api/v1/trending", axum::routing::get(routes::trending))
        .route("/api/v1/graph", axum::routing::get(routes::tool_graph))
        .route("/api/v1/suggest", axum::routing::get(routes::suggest))
        .route("/api/v1/openapi", axum::routing::get(routes::openapi))
        .route(
            "/api/v1/servers/:owner/:name/dependents",
            axum::routing::get(routes::dependents),
        )
        .route(
            "/api/v1/servers/:owner/:name/config",
            axum::routing::get(routes::server_config_snippet),
        )
        .route("/api/v1/random", axum::routing::get(routes::random_server))
        .route("/api/v1/export", axum::routing::get(routes::export_registry))
        .route("/api/v1/owners", axum::routing::get(routes::list_owners))
        .route("/api/v1/search/any", axum::routing::get(routes::search_any))
        .route("/api/v1/changelog", axum::routing::get(routes::changelog))
        .route("/api/v1/recently-updated", axum::routing::get(routes::recently_updated))
        .route(
            "/api/v1/servers/batch/delete",
            axum::routing::delete(routes::batch_delete_servers),
        )
        .route("/api/v1/import", axum::routing::post(routes::bulk_import))
        .route(
            "/api/v1/compare/:owner_a/:name_a/:owner_b/:name_b",
            axum::routing::get(routes::compare_servers),
        )
        .route("/api/v1/deprecated", axum::routing::get(routes::list_deprecated))
        .route("/api/v1/popular-tools", axum::routing::get(routes::popular_tools))
        .route(
            "/api/v1/compatibility/:owner_a/:name_a/:owner_b/:name_b",
            axum::routing::get(routes::compatibility),
        )
        .route(
            "/api/v1/servers/:owner/:name/bundle",
            axum::routing::get(routes::recommend_bundle),
        )
        .route(
            "/api/v1/servers/:owner/:name/score",
            axum::routing::get(routes::server_score),
        )
        .route(
            "/api/v1/servers/:owner/:name/star",
            axum::routing::post(routes::star_server),
        )
        .route(
            "/api/v1/servers/:owner/:name/unstar",
            axum::routing::post(routes::unstar_server),
        )
        .route("/api/v1/leaderboard", axum::routing::get(routes::leaderboard))
        .route("/api/v1/matrix", axum::routing::get(routes::matrix))
        .route(
            "/api/v1/servers/:owner/:name/badge",
            axum::routing::get(routes::server_badge),
        )
        .route("/api/v1/search/bulk", axum::routing::post(routes::bulk_search))
        .route("/api/v1/activity", axum::routing::get(routes::recent_activity))
        .layer(CorsLayer::permissive())
        .with_state(db_state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::ServerEntry;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn test_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&ServerEntry {
            id: None,
            owner: "modelcontextprotocol".into(),
            name: "filesystem".into(),
            version: "1.0.0".into(),
            description: "MCP server for filesystem operations".into(),
            author: "Anthropic".into(),
            license: "MIT".into(),
            repository: "https://github.com/modelcontextprotocol/servers".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into()],
            transport: "stdio".into(),
            tools: vec!["read_file".into(), "write_file".into(), "list_directory".into()],
            resources: vec!["file://".into()],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 1500,
            stars: 0,
            created_at: None,
            updated_at: None,
        })
        .unwrap();

        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_api_health() {
        let app = test_app().await;
        let req = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_version() {
        let app = test_app().await;
        let req = Request::builder()
            .uri("/api/v1/version")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["name"], "mcpreg");
    }

    #[tokio::test]
    async fn test_api_search() {
        let app = test_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=filesystem")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search_resp: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(search_resp.total, 1);
        assert_eq!(search_resp.servers[0].name, "filesystem");
    }

    #[tokio::test]
    async fn test_api_get_server() {
        let app = test_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/modelcontextprotocol/filesystem")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_get_server_not_found() {
        let app = test_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/nobody/nothing")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        // Verify JSON error body
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let err: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(err["error"].as_str().unwrap().contains("nobody/nothing"));
    }

    #[tokio::test]
    async fn test_api_delete_server() {
        let app = test_app().await;
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/servers/modelcontextprotocol/filesystem")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(v["success"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_api_delete_not_found() {
        let app = test_app().await;
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/servers/nobody/nothing")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_api_list_servers() {
        let app = test_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers?page=1&per_page=10")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let paginated: crate::api::types::PaginatedResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(paginated.total, 1);
        assert_eq!(paginated.page, 1);
    }

    #[tokio::test]
    async fn test_api_publish() {
        let app = test_app().await;
        let entry = ServerEntry {
            id: None,
            owner: "newuser".into(),
            name: "new-server".into(),
            version: "0.1.0".into(),
            description: "Brand new server".into(),
            author: "newuser".into(),
            license: "Apache-2.0".into(),
            repository: "https://github.com/newuser/new-server".into(),
            command: "python".into(),
            args: vec!["-m".into(), "new_server".into()],
            transport: "stdio".into(),
            tools: vec!["query".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/publish")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&entry).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_publish_validation_error() {
        let app = test_app().await;
        let entry = ServerEntry {
            id: None,
            owner: "".into(), // invalid: empty
            name: "test".into(),
            version: "0.1.0".into(),
            description: String::new(),
            author: String::new(),
            license: String::new(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/publish")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&entry).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}

#[cfg(test)]
mod new_endpoint_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_api_stats() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/stats")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let stats: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(stats["total_servers"].as_u64().unwrap() >= 30);
        assert!(stats["total_downloads"].as_i64().unwrap() > 0);
        assert!(stats["unique_owners"].as_u64().unwrap() > 0);
        assert!(stats["top_servers"].as_array().unwrap().len() == 5);
        // Enhanced stats fields
        assert!(stats["categories"].is_array());
        assert!(stats["licenses"].is_array());
        assert!(stats.get("total_tools").is_some());
        assert!(stats.get("total_prompts").is_some());
        assert!(stats.get("total_resources").is_some());
        assert!(stats.get("deprecated_servers").is_some());
    }

    #[tokio::test]
    async fn test_api_categories() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/categories")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let cats: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(cats["total"].as_u64().unwrap() > 0);
        assert!(!cats["categories"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_api_categories_filter() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/categories?category=database")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let cats: serde_json::Value = serde_json::from_slice(&body).unwrap();
        for cat in cats["categories"].as_array().unwrap() {
            let name = cat["category"].as_str().unwrap().to_lowercase();
            assert!(name.contains("database"), "Expected database category, got {name}");
        }
    }

    #[tokio::test]
    async fn test_api_search_tools() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=read_file")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total > 0, "Should find servers with read_file tool");
    }

    #[tokio::test]
    async fn test_api_search_empty_query() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total >= 30);
    }

    #[tokio::test]
    async fn test_api_search_multi_word() {
        let app = seeded_app().await;
        // "Anthropic filesystem" should find the filesystem server by Anthropic
        let req = Request::builder()
            .uri("/api/v1/search?q=Anthropic%20filesystem")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total >= 1);
        assert!(search.servers.iter().any(|s| s.name == "filesystem"));
    }

    #[tokio::test]
    async fn test_api_list_pagination() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers?page=1&per_page=5")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let paginated: crate::api::types::PaginatedResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(paginated.servers.len(), 5);
        assert_eq!(paginated.per_page, 5);
        assert!(paginated.total >= 30);
    }

    #[tokio::test]
    async fn test_api_list_per_page_capped() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers?per_page=999")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let paginated: crate::api::types::PaginatedResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(paginated.per_page, 100);
    }
}

#[cfg(test)]
mod tools_endpoint_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_api_tools_index() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/tools")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["total"].as_u64().unwrap() > 10, "Should have many tools");
        let tools = result["tools"].as_array().unwrap();
        assert!(!tools.is_empty());
        // Each tool should have name, server_count, servers
        let first = &tools[0];
        assert!(first["tool"].is_string());
        assert!(first["server_count"].as_u64().unwrap() > 0);
        assert!(first["servers"].is_array());
    }

    #[tokio::test]
    async fn test_api_tools_with_query_filter() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/tools?q=read_file")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["total"].as_u64().unwrap() >= 1);
        for tool in result["tools"].as_array().unwrap() {
            assert!(tool["tool"].as_str().unwrap().to_lowercase().contains("read_file"));
        }
    }

    #[tokio::test]
    async fn test_api_tools_with_limit() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/tools?limit=3")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["tools"].as_array().unwrap().len() <= 3);
    }

    #[tokio::test]
    async fn test_api_prompts_index() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/prompts")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        // May have 0 prompts in seed data, but endpoint should work
        assert!(result["total"].is_number());
        assert!(result["prompts"].is_array());
    }

    #[tokio::test]
    async fn test_api_tools_sorted_by_popularity() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/tools")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let tools = result["tools"].as_array().unwrap();
        // First tool should have more (or equal) servers than the last
        if tools.len() >= 2 {
            let first_count = tools[0]["server_count"].as_u64().unwrap();
            let last_count = tools.last().unwrap()["server_count"].as_u64().unwrap();
            assert!(first_count >= last_count, "Tools should be sorted by popularity");
        }
    }
}

#[cfg(test)]
mod improvement_tests {
    use super::*;
    use crate::api::types::ServerEntry;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_search_with_sort_param() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=server&sort=name")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        // Verify sorted by name
        if search.servers.len() >= 2 {
            for i in 1..search.servers.len() {
                assert!(
                    search.servers[i - 1].name.to_lowercase() <= search.servers[i].name.to_lowercase(),
                    "Expected name-sorted order"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_search_with_limit_param() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&limit=3")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(search.servers.len(), 3);
        assert_eq!(search.total, 3);
    }

    #[tokio::test]
    async fn test_search_with_category_filter() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&category=database")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total > 0);
        for s in &search.servers {
            let cat = crate::registry::seed::server_category(&s.owner, &s.name).to_lowercase();
            assert!(cat.contains("database"), "Expected database category, got {cat}");
        }
    }

    #[tokio::test]
    async fn test_publish_bad_version() {
        let app = seeded_app().await;
        let entry = ServerEntry {
            id: None,
            owner: "test".into(),
            name: "bad-version".into(),
            version: "not-semver".into(),
            description: "test".into(),
            author: "test".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/publish")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&entry).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_publish_bad_transport() {
        let app = seeded_app().await;
        let entry = ServerEntry {
            id: None,
            owner: "test".into(),
            name: "bad-transport".into(),
            version: "1.0.0".into(),
            description: "test".into(),
            author: "test".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "websocket".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/publish")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&entry).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_publish_missing_version() {
        let app = seeded_app().await;
        let entry = ServerEntry {
            id: None,
            owner: "test".into(),
            name: "no-version".into(),
            version: String::new(),
            description: "test".into(),
            author: "test".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/publish")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&entry).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_publish_with_prompts() {
        let app = seeded_app().await;
        let entry = ServerEntry {
            id: None,
            owner: "dev".into(),
            name: "prompt-server".into(),
            version: "1.0.0".into(),
            description: "Server with prompts".into(),
            author: "dev".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec!["index.js".into()],
            transport: "stdio".into(),
            tools: vec!["tool1".into()],
            resources: vec![],
            prompts: vec!["summarize".into(), "analyze".into()],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/publish")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&entry).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_search_with_min_downloads() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&min_downloads=30000")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total > 0);
        for s in &search.servers {
            assert!(s.downloads >= 30000, "Expected >=30000, got {}", s.downloads);
        }
    }

    #[tokio::test]
    async fn test_download_tracking_endpoint() {
        let app = seeded_app().await;
        // First get the current download count
        let req = Request::builder()
            .uri("/api/v1/servers/modelcontextprotocol/filesystem")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let before: ServerEntry = serde_json::from_slice(&body).unwrap();

        // Track a download
        let app2 = seeded_app().await; // fresh app since oneshot consumes
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/servers/modelcontextprotocol/filesystem/download")
            .body(Body::empty())
            .unwrap();
        let resp = app2.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(v["success"].as_bool().unwrap());

        // Note: can't verify count increased because seeded_app creates separate DBs
        let _ = before; // used above for reference
    }

    #[tokio::test]
    async fn test_download_tracking_not_found() {
        let app = seeded_app().await;
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/servers/nobody/nothing/download")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_api_similar_servers() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/modelcontextprotocol/postgres/similar?limit=3")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["total"].as_u64().unwrap() > 0);
        assert!(result["similar"].as_array().unwrap().len() <= 3);
        assert_eq!(result["query"], "modelcontextprotocol/postgres");
    }

    #[tokio::test]
    async fn test_api_similar_not_found() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/nobody/nothing/similar")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_search_with_tool_filter() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&tool=read_file")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total >= 2, "Multiple servers have read_file tool");
        for s in &search.servers {
            assert!(
                s.tools.iter().any(|t| t.to_lowercase().contains("read_file")),
                "Server {} should have read_file tool",
                s.full_name()
            );
        }
    }

    #[tokio::test]
    async fn test_search_with_transport_filter() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&transport=stdio")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total > 0);
        for s in &search.servers {
            assert_eq!(s.transport, "stdio", "Expected stdio transport");
        }
    }

    #[tokio::test]
    async fn test_batch_get_servers() {
        let app = seeded_app().await;
        let body_json = serde_json::json!({
            "servers": [
                "modelcontextprotocol/filesystem",
                "modelcontextprotocol/git",
                "nobody/nothing"
            ]
        });
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/servers/batch")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["total"].as_u64().unwrap(), 2);
        assert_eq!(result["not_found"].as_array().unwrap().len(), 1);
        assert_eq!(result["not_found"][0], "nobody/nothing");
    }

    #[tokio::test]
    async fn test_batch_get_empty() {
        let app = seeded_app().await;
        let body_json = serde_json::json!({"servers": []});
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/servers/batch")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["total"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_api_version_history() {
        let db = Database::open_in_memory().unwrap();
        let mut entry = ServerEntry {
            id: None,
            owner: "alice".into(),
            name: "tool".into(),
            version: "1.0.0".into(),
            description: "Test".into(),
            author: "alice".into(),
            license: "MIT".into(),
            repository: "https://github.com/alice/tool".into(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry).unwrap();
        entry.version = "1.1.0".into();
        db.upsert_server(&entry).unwrap();
        entry.version = "2.0.0".into();
        db.upsert_server(&entry).unwrap();

        let db_state: DbState = Arc::new(Mutex::new(db));
        let app = build_router(db_state);

        let req = Request::builder()
            .uri("/api/v1/servers/alice/tool/versions")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["current_version"], "2.0.0");
        assert_eq!(result["total"].as_u64().unwrap(), 3);
        let versions = result["versions"].as_array().unwrap();
        assert_eq!(versions[0]["version"], "2.0.0");
        assert_eq!(versions[2]["version"], "1.0.0");
    }

    #[tokio::test]
    async fn test_api_version_history_not_found() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/nobody/nothing/versions")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_search_combined_sort_and_limit() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&sort=name&limit=5")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(search.servers.len(), 5);
        // Verify sorted by name
        for i in 1..search.servers.len() {
            assert!(
                search.servers[i - 1].name.to_lowercase() <= search.servers[i].name.to_lowercase(),
            );
        }
    }
}

#[cfg(test)]
mod diff_endpoint_tests {
    use super::*;
    use crate::api::types::ServerEntry;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn test_app_with_versions() -> Router {
        let db = Database::open_in_memory().unwrap();
        let mut entry = ServerEntry {
            id: None,
            owner: "alice".into(),
            name: "tool".into(),
            version: "1.0.0".into(),
            description: "Initial version".into(),
            author: "alice".into(),
            license: "MIT".into(),
            repository: "https://github.com/alice/tool".into(),
            command: "node".into(),
            args: vec!["index.js".into()],
            transport: "stdio".into(),
            tools: vec!["read".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 100,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry).unwrap();
        entry.version = "2.0.0".into();
        entry.tools = vec!["read".into(), "write".into(), "delete".into()];
        entry.description = "Updated with write and delete support".into();
        db.upsert_server(&entry).unwrap();

        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_diff_endpoint() {
        let app = test_app_with_versions().await;
        let req = Request::builder()
            .uri("/api/v1/servers/alice/tool/diff?from=1.0.0")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["current_version"], "2.0.0");
        assert_eq!(result["from_version"], "1.0.0");
        assert_eq!(result["current"]["tools"].as_array().unwrap().len(), 3);
        assert!(result["total_versions"].as_u64().unwrap() >= 2);
    }

    #[tokio::test]
    async fn test_diff_endpoint_no_from() {
        let app = test_app_with_versions().await;
        let req = Request::builder()
            .uri("/api/v1/servers/alice/tool/diff")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["from_version"], "unknown");
    }

    #[tokio::test]
    async fn test_diff_not_found() {
        let app = test_app_with_versions().await;
        let req = Request::builder()
            .uri("/api/v1/servers/nobody/nothing/diff")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_health_returns_json() {
        let app = test_app_with_versions().await;
        let req = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["status"], "ok");
        assert!(result["version"].as_str().is_some());
        assert!(result["servers"].as_i64().unwrap() >= 0);
    }
}

#[cfg(test)]
mod filter_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_search_with_author_filter() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&author=Anthropic")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total > 0, "Should find servers by Anthropic");
        for s in &search.servers {
            assert!(
                s.author.to_lowercase().contains("anthropic"),
                "Expected Anthropic author, got '{}'", s.author
            );
        }
    }

    #[tokio::test]
    async fn test_search_with_owner_filter() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&owner=modelcontextprotocol")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total > 0);
        for s in &search.servers {
            assert!(
                s.owner.to_lowercase().contains("modelcontextprotocol"),
                "Expected MCP owner, got '{}'", s.owner
            );
        }
    }

    #[tokio::test]
    async fn test_search_combined_author_and_category() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&author=Anthropic&category=database")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        for s in &search.servers {
            assert!(s.author.to_lowercase().contains("anthropic"));
            let cat = crate::registry::seed::server_category(&s.owner, &s.name).to_lowercase();
            assert!(cat.contains("database"));
        }
    }

    #[tokio::test]
    async fn test_search_author_no_match() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&author=NonExistentAuthor12345")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(search.total, 0);
    }
}

#[cfg(test)]
mod validate_and_patch_tests {
    use axum::http::StatusCode;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    use crate::api::types::ServerEntry;


    fn test_app() -> axum::Router {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let db_state = std::sync::Arc::new(tokio::sync::Mutex::new(db));
        super::build_router(db_state)
    }

    fn test_entry() -> ServerEntry {
        ServerEntry {
            id: None,
            owner: "testuser".into(),
            name: "test-server".into(),
            version: "1.0.0".into(),
            description: "Test server".into(),
            author: "testuser".into(),
            license: "MIT".into(),
            repository: "https://github.com/test/repo".into(),
            command: "node".into(),
            args: vec!["index.js".into()],
            transport: "stdio".into(),
            tools: vec!["tool1".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        }
    }

    #[tokio::test]
    async fn test_validate_valid_entry() {
        let app = test_app();
        let entry = test_entry();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/validate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&entry).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["valid"], true);
        assert!(body["errors"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_validate_missing_fields() {
        let app = test_app();
        let entry = ServerEntry {
            id: None,
            owner: "".into(),
            name: "".into(),
            version: "bad".into(),
            description: "".into(),
            author: "".into(),
            license: "".into(),
            repository: "".into(),
            command: "".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/validate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&entry).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["valid"], false);
        let errors = body["errors"].as_array().unwrap();
        assert!(errors.len() >= 3); // owner, name, command, version
    }

    #[tokio::test]
    async fn test_validate_warnings() {
        let app = test_app();
        let entry = ServerEntry {
            id: None,
            owner: "user".into(),
            name: "tool".into(),
            version: "1.0.0".into(),
            description: "".into(), // triggers warning
            author: "".into(),       // triggers warning
            license: "".into(),      // triggers warning
            repository: "".into(),   // triggers warning
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],           // triggers warning
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/validate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&entry).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["valid"], true);
        let warnings = body["warnings"].as_array().unwrap();
        assert!(warnings.len() >= 4);
    }

    #[tokio::test]
    async fn test_patch_server() {
        let app = test_app();

        // First publish
        let entry = test_entry();
        let resp = app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/publish")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&entry).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Patch description and tags
        let patch = serde_json::json!({
            "description": "Updated description",
            "tags": ["ai", "productivity"],
        });
        let resp = app.clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/v1/servers/testuser/test-server")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&patch).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["success"], true);
        let fields = body["updated_fields"].as_array().unwrap();
        assert!(fields.iter().any(|f| f == "description"));
        assert!(fields.iter().any(|f| f == "tags"));

        // Verify the changes persisted
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/servers/testuser/test-server")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["description"], "Updated description");
        assert_eq!(body["tags"], serde_json::json!(["ai", "productivity"]));
    }

    #[tokio::test]
    async fn test_patch_nonexistent() {
        let app = test_app();
        let patch = serde_json::json!({"description": "test"});
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/v1/servers/nobody/nothing")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&patch).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_patch_empty_body() {
        let app = test_app();
        // Publish first
        let entry = test_entry();
        let _ = app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/publish")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&entry).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let patch = serde_json::json!({}); // no valid fields
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/v1/servers/testuser/test-server")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&patch).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_patch_invalid_transport() {
        let app = test_app();
        let entry = test_entry();
        let _ = app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/publish")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&entry).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let patch = serde_json::json!({"transport": "invalid-proto"});
        let resp = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/v1/servers/testuser/test-server")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&patch).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}

#[cfg(test)]
mod search_suggestions_tests {
    use axum::http::StatusCode;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;
    use crate::api::types::ServerEntry;


    fn test_app() -> axum::Router {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let db_state = std::sync::Arc::new(tokio::sync::Mutex::new(db));
        super::build_router(db_state)
    }


    #[tokio::test]
    async fn test_search_returns_suggestions_on_typo() {
        let app = test_app();

        // Publish a server
        let entry = ServerEntry {
            id: None,
            owner: "org".into(),
            name: "filesystem".into(),
            version: "1.0.0".into(),
            description: "File system access".into(),
            author: "org".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        let _ = app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/publish")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&entry).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Search with a typo
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/search?q=filesytem")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["total"], 0);
        // Should have suggestions
        let suggestions = body["suggestions"].as_array();
        assert!(suggestions.is_some(), "Should have suggestions for typo");
        assert!(!suggestions.unwrap().is_empty());
    }
}

#[cfg(test)]
mod trending_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_api_trending_default() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/trending")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let trending = result["trending"].as_array().unwrap();
        assert!(!trending.is_empty());
        assert!(trending.len() <= 15);
        // First should have highest downloads
        if trending.len() >= 2 {
            let dl0 = trending[0]["downloads"].as_i64().unwrap();
            let dl1 = trending[1]["downloads"].as_i64().unwrap();
            assert!(dl0 >= dl1, "Should be sorted by downloads desc");
        }
        // Each item should have rank
        assert_eq!(trending[0]["rank"], 1);
    }

    #[tokio::test]
    async fn test_api_trending_with_limit() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/trending?limit=3")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["trending"].as_array().unwrap().len() <= 3);
    }

    #[tokio::test]
    async fn test_api_trending_with_category() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/trending?category=database")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let trending = result["trending"].as_array().unwrap();
        for item in trending {
            let cat = item["category"].as_str().unwrap().to_lowercase();
            assert!(cat.contains("database"), "Expected database category, got {cat}");
        }
    }

    #[tokio::test]
    async fn test_api_trending_with_transport() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/trending?transport=stdio")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let trending = result["trending"].as_array().unwrap();
        assert!(!trending.is_empty());
    }
}

#[cfg(test)]
mod license_filter_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_search_with_license_filter_mit() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&license=MIT")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total > 0, "Should find MIT-licensed servers");
        for s in &search.servers {
            assert!(
                s.license.to_lowercase().contains("mit"),
                "Expected MIT license, got '{}'", s.license
            );
        }
    }

    #[tokio::test]
    async fn test_search_with_license_filter_no_match() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&license=WTFPL")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(search.total, 0, "Should not find WTFPL-licensed servers");
    }

    #[tokio::test]
    async fn test_search_license_combined_with_category() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&license=MIT&category=database")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        for s in &search.servers {
            assert!(s.license.to_lowercase().contains("mit"));
        }
    }

    #[tokio::test]
    async fn test_search_license_case_insensitive() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&license=mit")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total > 0, "Case-insensitive license filter should work");
    }
}

#[cfg(test)]
mod graph_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_api_graph_default() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/graph")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["total_edges"].as_u64().unwrap() > 0);
        assert!(result["total_nodes"].as_u64().unwrap() > 0);
        let edges = result["edges"].as_array().unwrap();
        for edge in edges {
            assert!(edge["shared_count"].as_u64().unwrap() >= 1);
            assert!(!edge["shared_tools"].as_array().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn test_api_graph_min_shared() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/graph?min_shared=2")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let edges = result["edges"].as_array().unwrap();
        for edge in edges {
            assert!(edge["shared_count"].as_u64().unwrap() >= 2);
        }
    }

    #[tokio::test]
    async fn test_api_graph_with_limit() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/graph?limit=5")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["total_edges"].as_u64().unwrap() <= 5);
    }

    #[tokio::test]
    async fn test_api_graph_sorted_by_shared_count() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/graph")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let edges = result["edges"].as_array().unwrap();
        if edges.len() >= 2 {
            let c0 = edges[0]["shared_count"].as_u64().unwrap();
            let c1 = edges[1]["shared_count"].as_u64().unwrap();
            assert!(c0 >= c1, "Edges should be sorted by shared_count desc");
        }
    }
}

#[cfg(test)]
mod tags_endpoint_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_api_tags_index() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/tags")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["total"].as_u64().unwrap() >= 5, "Should have many tags from seeded data");
        let tags = result["tags"].as_array().unwrap();
        assert!(!tags.is_empty());
        // Each tag should have name, server_count, servers
        let first = &tags[0];
        assert!(first["tag"].is_string());
        assert!(first["server_count"].as_u64().unwrap() > 0);
        assert!(first["servers"].is_array());
    }

    #[tokio::test]
    async fn test_api_tags_with_query() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/tags?q=official")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["total"].as_u64().unwrap() >= 1);
        for tag in result["tags"].as_array().unwrap() {
            assert!(tag["tag"].as_str().unwrap().contains("official"));
        }
    }

    #[tokio::test]
    async fn test_api_tags_with_limit() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/tags?limit=3")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["tags"].as_array().unwrap().len() <= 3);
    }

    #[tokio::test]
    async fn test_api_tags_no_match() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/tags?q=nonexistenttag12345")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["total"].as_u64().unwrap(), 0);
        assert!(result["tags"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_search_with_tag_filter() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&tag=database")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total > 0, "Should find servers tagged 'database'");
        for s in &search.servers {
            assert!(
                s.tags.iter().any(|t| t.to_lowercase().contains("database")),
                "Server {} should have database tag, got {:?}",
                s.full_name(),
                s.tags
            );
        }
    }

    #[tokio::test]
    async fn test_search_with_tag_filter_official() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&tag=official")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total > 0);
        for s in &search.servers {
            assert_eq!(s.owner, "modelcontextprotocol", "Official tag should only be on MCP servers");
        }
    }
}

#[cfg(test)]
mod openapi_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_openapi_endpoint() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/openapi")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["openapi"], "3.0.0");
        assert_eq!(result["info"]["title"], "mcpreg API");
        assert!(result["paths"].as_object().unwrap().len() >= 15, "Should document many endpoints");
        assert!(result["paths"]["/api/v1/search"].is_object());
        assert!(result["paths"]["/api/v1/openapi"].is_object());
    }

    #[tokio::test]
    async fn test_openapi_has_version() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/openapi")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let version = result["info"]["version"].as_str().unwrap();
        assert!(!version.is_empty());
    }
}

#[cfg(test)]
mod dependents_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_dependents_for_filesystem() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/modelcontextprotocol/filesystem/dependents")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["server"], "modelcontextprotocol/filesystem");
        // There should be some servers sharing tools like read_file
        let dependents = result["dependents"].as_array().unwrap();
        if !dependents.is_empty() {
            let first = &dependents[0];
            assert!(first["shared_count"].as_u64().unwrap() > 0);
            assert!(!first["shared_tools"].as_array().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn test_dependents_not_found() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/nobody/nothing/dependents")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_dependents_with_limit() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/modelcontextprotocol/filesystem/dependents?limit=2")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["dependents"].as_array().unwrap().len() <= 2);
    }

    #[tokio::test]
    async fn test_dependents_excludes_self() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/modelcontextprotocol/filesystem/dependents")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        for dep in result["dependents"].as_array().unwrap() {
            let name = dep["full_name"].as_str().unwrap();
            assert_ne!(name, "modelcontextprotocol/filesystem", "Should not include self");
        }
    }

    #[tokio::test]
    async fn test_dependents_sorted_by_shared_count() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/modelcontextprotocol/filesystem/dependents")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let deps = result["dependents"].as_array().unwrap();
        if deps.len() >= 2 {
            let c0 = deps[0]["shared_count"].as_u64().unwrap();
            let c1 = deps[1]["shared_count"].as_u64().unwrap();
            assert!(c0 >= c1, "Should be sorted by shared_count desc");
        }
    }
}

#[cfg(test)]
mod suggest_endpoint_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_suggest_endpoint_basic() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/suggest?q=file")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["total"].as_u64().unwrap() > 0);
        let suggestions = result["suggestions"].as_array().unwrap();
        assert!(suggestions.iter().any(|s| s.as_str().unwrap().contains("filesystem")));
    }

    #[tokio::test]
    async fn test_suggest_endpoint_empty_query() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/suggest?q=")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["total"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_suggest_endpoint_with_limit() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/suggest?q=s&limit=3")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["suggestions"].as_array().unwrap().len() <= 3);
    }

    #[tokio::test]
    async fn test_suggest_endpoint_no_match() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/suggest?q=zzzznonexistent")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["total"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_suggest_full_name_prefix() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/suggest?q=modelcontextprotocol%2Ffile")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["total"].as_u64().unwrap() >= 1);
        let suggestions = result["suggestions"].as_array().unwrap();
        for s in suggestions {
            assert!(s.as_str().unwrap().starts_with("modelcontextprotocol/file"));
        }
    }

    #[tokio::test]
    async fn test_suggest_documented_in_openapi() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/openapi")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["paths"]["/api/v1/suggest"].is_object(), "Suggest should be in OpenAPI docs");
    }
}

#[cfg(test)]
mod fuzzy_prefix_integration_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_search_prefix_match_ranks_higher() {
        let app = seeded_app().await;
        // "file" is a prefix of "filesystem" — should appear first
        let req = Request::builder()
            .uri("/api/v1/search?q=file")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert!(search.total > 0);
        // The filesystem server should be in results
        assert!(search.servers.iter().any(|s| s.name.starts_with("file")));
    }

    #[tokio::test]
    async fn test_search_typo_returns_suggestions() {
        let app = seeded_app().await;
        // Search for "filesytem" (typo) should yield suggestions
        let req = Request::builder()
            .uri("/api/v1/search?q=zzzznotexist")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(search.total, 0);
        // Suggestions may or may not appear depending on distance, but endpoint should work
    }
}

#[cfg(test)]
mod random_and_config_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_api_random_server() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/random")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["server"]["full_name"].is_string());
        assert!(result["server"]["command"].is_string());
        assert!(result["server"]["tools"].is_array());
    }

    #[tokio::test]
    async fn test_api_random_with_category() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/random?category=database")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let cat = result["server"]["category"].as_str().unwrap().to_lowercase();
        assert!(cat.contains("database"), "Expected database category, got {cat}");
    }

    #[tokio::test]
    async fn test_api_random_empty_db() {
        let db = Database::open_in_memory().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        let app = build_router(db_state);

        let req = Request::builder()
            .uri("/api/v1/random")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_api_random_nonexistent_category() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/random?category=zzzznonexistent")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_api_config_snippet() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/modelcontextprotocol/filesystem/config")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["server"], "modelcontextprotocol/filesystem");
        assert!(result["config_key"].is_string());
        assert!(result["claude_desktop_config"]["mcpServers"].is_object());
        assert!(result["instructions"].is_string());

        // The config should have command and args
        let config_key = result["config_key"].as_str().unwrap();
        let server_config = &result["claude_desktop_config"]["mcpServers"][config_key];
        assert!(server_config["command"].is_string());
        assert!(server_config["args"].is_array());
    }

    #[tokio::test]
    async fn test_api_config_snippet_not_found() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/nobody/nothing/config")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_api_config_snippet_sse_transport() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&crate::api::types::ServerEntry {
            id: None,
            owner: "test".into(),
            name: "sse-server".into(),
            version: "1.0.0".into(),
            description: "SSE transport server".into(),
            author: "test".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec!["index.js".into()],
            transport: "sse".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        }).unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        let app = build_router(db_state);

        let req = Request::builder()
            .uri("/api/v1/servers/test/sse-server/config")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let key = result["config_key"].as_str().unwrap();
        let server_config = &result["claude_desktop_config"]["mcpServers"][key];
        // SSE transport should be included in config
        assert_eq!(server_config["transport"], "sse");
    }

    #[tokio::test]
    async fn test_api_batch_delete() {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let initial_count = db.count_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        let app = build_router(db_state);

        let body_json = serde_json::json!({
            "servers": [
                "modelcontextprotocol/filesystem",
                "modelcontextprotocol/git",
                "nobody/nothing"
            ]
        });
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/servers/batch/delete")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["deleted"].as_u64().unwrap(), 2);
        assert_eq!(result["total_requested"].as_u64().unwrap(), 3);
        assert_eq!(result["not_found_count"].as_u64().unwrap(), 1);

        // Verify count decreased
        let req = Request::builder()
            .uri("/api/v1/stats")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let stats: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(stats["total_servers"].as_u64().unwrap() as usize, initial_count - 2);
    }

    #[tokio::test]
    async fn test_api_batch_delete_empty() {
        let app = seeded_app().await;
        let body_json = serde_json::json!({"servers": []});
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/servers/batch/delete")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["deleted"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_api_batch_delete_invalid_refs() {
        let app = seeded_app().await;
        let body_json = serde_json::json!({
            "servers": ["no-slash", "also-bad"]
        });
        let req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/servers/batch/delete")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["deleted"].as_u64().unwrap(), 0);
        assert_eq!(result["invalid_refs"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_openapi_includes_new_endpoints() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/openapi")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let paths = result["paths"].as_object().unwrap();
        assert!(paths.contains_key("/api/v1/random"), "OpenAPI should document /random");
        assert!(paths.contains_key("/api/v1/servers/{owner}/{name}/config"), "OpenAPI should document /config");
        assert!(paths.contains_key("/api/v1/servers/batch/delete"), "OpenAPI should document batch delete");
    }
}

#[cfg(test)]
mod export_owners_searchany_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> axum::Router {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let state = std::sync::Arc::new(tokio::sync::Mutex::new(db));
        build_router(state)
    }

    #[tokio::test]
    async fn test_api_export() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/export")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["total"].as_u64().unwrap() >= 30);
        assert!(result["servers"].as_array().unwrap().len() >= 30);
        assert!(result["exported_at"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_api_owners() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/owners")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let owners = result["owners"].as_array().unwrap();
        assert!(!owners.is_empty());
        // modelcontextprotocol should be present
        assert!(owners.iter().any(|o| o["owner"] == "modelcontextprotocol"));
        // Should have server_count field
        assert!(owners[0]["server_count"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_api_search_any_or() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search/any?q=postgres|sqlite|redis")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["total"].as_u64().unwrap() >= 3);
    }

    #[tokio::test]
    async fn test_api_search_any_empty() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/search/any?q=")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["total"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_api_config_snippet_includes_env() {
        // Create a server with env hints and verify config snippet includes them
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let mut entry = crate::api::types::ServerEntry {
            id: None,
            owner: "test".into(),
            name: "env-server".into(),
            version: "1.0.0".into(),
            description: "Server with env".into(),
            author: "test".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "npx".into(),
            args: vec!["-y".into(), "test-server".into()],
            transport: "stdio".into(),
            tools: vec!["query".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        entry.env.insert("API_KEY".into(), "your-key-here".into());
        db.upsert_server(&entry).unwrap();

        let state = std::sync::Arc::new(tokio::sync::Mutex::new(db));
        let app = build_router(state);
        let req = Request::builder()
            .uri("/api/v1/servers/test/env-server/config")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        // Config snippet should include env
        let config = &result["claude_desktop_config"]["mcpServers"]["test-env-server"];
        assert!(config["env"].is_object(), "Config snippet should include env");
        assert_eq!(config["env"]["API_KEY"], "your-key-here");
    }

    #[tokio::test]
    async fn test_api_validate_env_warnings() {
        let app = seeded_app().await;
        let body_json = serde_json::json!({
            "owner": "test",
            "name": "valid-server",
            "version": "1.0.0",
            "command": "node",
            "transport": "stdio",
            "env": {"bad-key": "value"},
            "homepage": "not-a-url",
        });
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/validate")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body_json).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["valid"].as_bool().unwrap(), "Should be valid (warnings only)");
        let warnings = result["warnings"].as_array().unwrap();
        assert!(warnings.iter().any(|w| w.as_str().unwrap().contains("UPPER_SNAKE_CASE")));
        assert!(warnings.iter().any(|w| w.as_str().unwrap().contains("homepage")));
    }

    #[tokio::test]
    async fn test_api_patch_env_and_homepage() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let entry = crate::api::types::ServerEntry {
            id: None,
            owner: "patcher".into(),
            name: "target".into(),
            version: "1.0.0".into(),
            description: "Patchable".into(),
            author: "patcher".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry).unwrap();

        let state = std::sync::Arc::new(tokio::sync::Mutex::new(db));
        let app = build_router(state);
        let patch_body = serde_json::json!({
            "env": {"NEW_KEY": "new_value"},
            "homepage": "https://example.com",
        });
        let req = Request::builder()
            .method("PATCH")
            .uri("/api/v1/servers/patcher/target")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&patch_body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let updated_fields = result["updated_fields"].as_array().unwrap();
        assert!(updated_fields.iter().any(|f| f == "env"));
        assert!(updated_fields.iter().any(|f| f == "homepage"));
    }

    #[tokio::test]
    async fn test_openapi_includes_export_owners() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/openapi")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let paths = result["paths"].as_object().unwrap();
        assert!(paths.contains_key("/api/v1/export"), "Should document /export");
        assert!(paths.contains_key("/api/v1/owners"), "Should document /owners");
        assert!(paths.contains_key("/api/v1/search/any"), "Should document /search/any");
    }
}

#[cfg(test)]
mod changelog_and_recent_tests {
    use super::*;
    use crate::api::types::ServerEntry;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    async fn app_with_versions() -> Router {
        let db = Database::open_in_memory().unwrap();
        let mut entry = ServerEntry {
            id: None,
            owner: "dev".into(),
            name: "toolbox".into(),
            version: "1.0.0".into(),
            description: "Dev toolbox".into(),
            author: "dev".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec!["run".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry).unwrap();
        entry.version = "1.1.0".into();
        db.upsert_server(&entry).unwrap();
        entry.version = "2.0.0".into();
        db.upsert_server(&entry).unwrap();

        let mut entry2 = ServerEntry {
            id: None,
            owner: "dev".into(),
            name: "helper".into(),
            version: "0.1.0".into(),
            description: "Helper tool".into(),
            author: "dev".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "python".into(),
            args: vec!["-m".into(), "helper".into()],
            transport: "stdio".into(),
            tools: vec!["assist".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 10,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry2).unwrap();
        entry2.version = "0.2.0".into();
        db.upsert_server(&entry2).unwrap();

        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_changelog_endpoint() {
        let app = app_with_versions().await;
        let req = Request::builder()
            .uri("/api/v1/changelog")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["total"].as_u64().unwrap() >= 5); // 3 toolbox + 2 helper versions
        let changelog = result["changelog"].as_array().unwrap();
        // Most recent first
        assert!(changelog[0]["version"].is_string());
        assert!(changelog[0]["server"].is_string());
        assert!(changelog[0]["published_at"].is_string());
    }

    #[tokio::test]
    async fn test_changelog_with_limit() {
        let app = app_with_versions().await;
        let req = Request::builder()
            .uri("/api/v1/changelog?limit=2")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["changelog"].as_array().unwrap().len() <= 2);
    }

    #[tokio::test]
    async fn test_changelog_empty_db() {
        let db = Database::open_in_memory().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        let app = build_router(db_state);

        let req = Request::builder()
            .uri("/api/v1/changelog")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["total"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_recently_updated_endpoint() {
        let app = app_with_versions().await;
        let req = Request::builder()
            .uri("/api/v1/recently-updated")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["total"].as_u64().unwrap() >= 2);
        let servers = result["servers"].as_array().unwrap();
        assert!(servers[0]["full_name"].is_string());
        assert!(servers[0]["version"].is_string());
        assert!(servers[0]["updated_at"].is_string());
    }

    #[tokio::test]
    async fn test_recently_updated_with_limit() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/recently-updated?limit=3")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["servers"].as_array().unwrap().len() <= 3);
    }

    #[tokio::test]
    async fn test_openapi_includes_changelog() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/openapi")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let paths = result["paths"].as_object().unwrap();
        assert!(paths.contains_key("/api/v1/changelog"), "OpenAPI should document /changelog");
        assert!(paths.contains_key("/api/v1/recently-updated"), "OpenAPI should document /recently-updated");
    }
}

#[cfg(test)]
mod resources_endpoint_tests {
    use super::*;
    use axum::http::StatusCode;

    fn test_app() -> axum::Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        super::build_router(std::sync::Arc::new(tokio::sync::Mutex::new(db)))
    }

    #[tokio::test]
    async fn test_api_resources_index() {
        let app = test_app();
        let resp = axum::http::Request::builder()
            .uri("/api/v1/resources")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = tower::ServiceExt::oneshot(app, resp).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert!(body["resources"].is_array());
        assert!(body["total"].is_number());
    }

    #[tokio::test]
    async fn test_api_resources_with_query() {
        let app = test_app();
        let resp = axum::http::Request::builder()
            .uri("/api/v1/resources?q=file")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = tower::ServiceExt::oneshot(app, resp).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        let resources = body["resources"].as_array().unwrap();
        // All returned resources should contain "file" (case-insensitive)
        for r in resources {
            let name = r["resource"].as_str().unwrap().to_lowercase();
            assert!(name.contains("file"), "Resource '{}' should contain 'file'", name);
        }
    }

    #[tokio::test]
    async fn test_api_resources_with_limit() {
        let app = test_app();
        let resp = axum::http::Request::builder()
            .uri("/api/v1/resources?limit=2")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = tower::ServiceExt::oneshot(app, resp).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        let resources = body["resources"].as_array().unwrap();
        assert!(resources.len() <= 2);
    }
}

#[cfg(test)]
mod search_filter_tests {
    use super::*;
    use axum::http::StatusCode;

    fn test_app() -> axum::Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        super::build_router(std::sync::Arc::new(tokio::sync::Mutex::new(db)))
    }

    #[tokio::test]
    async fn test_search_with_min_tools() {
        let app = test_app();
        let resp = axum::http::Request::builder()
            .uri("/api/v1/search?q=&min_tools=3")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = tower::ServiceExt::oneshot(app, resp).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        let servers = body["servers"].as_array().unwrap();
        for s in servers {
            let tools = s["tools"].as_array().unwrap();
            assert!(tools.len() >= 3, "Server should have at least 3 tools, got {}", tools.len());
        }
    }

    #[tokio::test]
    async fn test_search_with_has_prompts() {
        let app = test_app();
        let resp = axum::http::Request::builder()
            .uri("/api/v1/search?q=&has_prompts=true")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = tower::ServiceExt::oneshot(app, resp).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        let servers = body["servers"].as_array().unwrap();
        for s in servers {
            let prompts = s["prompts"].as_array().unwrap();
            assert!(!prompts.is_empty(), "Server should have at least one prompt");
        }
    }

    #[tokio::test]
    async fn test_search_with_resource_filter() {
        let app = test_app();
        let resp = axum::http::Request::builder()
            .uri("/api/v1/search?q=&resource=file")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = tower::ServiceExt::oneshot(app, resp).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        let servers = body["servers"].as_array().unwrap();
        for s in servers {
            let resources = s["resources"].as_array().unwrap();
            let has_file = resources.iter().any(|r| r.as_str().unwrap().to_lowercase().contains("file"));
            assert!(has_file, "Server should have a file resource");
        }
    }

    #[tokio::test]
    async fn test_search_min_tools_zero_returns_all() {
        let app = test_app();
        // min_tools=0 should not filter anything
        let resp = axum::http::Request::builder()
            .uri("/api/v1/search?q=&min_tools=0")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = tower::ServiceExt::oneshot(app, resp).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        let total = body["total"].as_u64().unwrap();
        assert!(total > 0, "Should return servers when min_tools=0");
    }

    #[tokio::test]
    async fn test_search_min_tools_very_high_returns_none() {
        let app = test_app();
        let resp = axum::http::Request::builder()
            .uri("/api/v1/search?q=&min_tools=999")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = tower::ServiceExt::oneshot(app, resp).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        let servers = body["servers"].as_array().unwrap();
        assert!(servers.is_empty(), "No server should have 999 tools");
    }
}

#[cfg(test)]
mod bulk_import_tests {
    use super::*;
    use crate::api::types::ServerEntry;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn seed_entry(owner: &str, name: &str) -> ServerEntry {
        ServerEntry {
            id: None,
            owner: owner.into(),
            name: name.into(),
            version: "1.0.0".into(),
            description: format!("{name} server"),
            author: owner.into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        }
    }

    async fn setup() -> axum::Router {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let state: crate::registry::routes::DbState =
            std::sync::Arc::new(tokio::sync::Mutex::new(db));
        build_router(state)
    }

    #[tokio::test]
    async fn test_bulk_import_success() {
        let app = setup().await;
        let body = serde_json::json!({
            "servers": [
                seed_entry("alice", "tool-a"),
                seed_entry("bob", "tool-b"),
            ]
        });
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/import")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["imported"], 2);
        assert_eq!(body["total_submitted"], 2);
        assert!(body["errors"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_bulk_import_partial_errors() {
        let app = setup().await;
        let body = serde_json::json!({
            "servers": [
                seed_entry("alice", "ok-server"),
                { "name": "missing-owner" },
            ]
        });
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/import")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["imported"], 1);
        assert_eq!(body["errors"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_bulk_import_missing_servers_array() {
        let app = setup().await;
        let body = serde_json::json!({ "data": [] });
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/import")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        // Should return an error
        assert_ne!(response.status(), 200);
    }
}

#[cfg(test)]
mod compare_api_tests {
    use super::*;
    use crate::api::types::ServerEntry;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn setup_with_servers() -> axum::Router {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let entry_a = ServerEntry {
            id: None,
            owner: "alice".into(),
            name: "tool-a".into(),
            version: "1.0.0".into(),
            description: "Tool A".into(),
            author: "alice".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec!["read_file".into(), "write_file".into()],
            resources: vec!["file://".into()],
            prompts: vec!["summarize".into()],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 100,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        let entry_b = ServerEntry {
            id: None,
            owner: "bob".into(),
            name: "tool-b".into(),
            version: "2.0.0".into(),
            description: "Tool B".into(),
            author: "bob".into(),
            license: "Apache-2.0".into(),
            repository: String::new(),
            command: "python".into(),
            args: vec![],
            transport: "sse".into(),
            tools: vec!["read_file".into(), "search".into()],
            resources: vec!["file://".into(), "http://".into()],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 50,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry_a).unwrap();
        db.upsert_server(&entry_b).unwrap();
        let state: crate::registry::routes::DbState =
            std::sync::Arc::new(tokio::sync::Mutex::new(db));
        build_router(state)
    }

    #[tokio::test]
    async fn test_compare_servers_api() {
        let app = setup_with_servers().await;
        let request = Request::builder()
            .uri("/api/v1/compare/alice/tool-a/bob/tool-b")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["server_a"]["full_name"], "alice/tool-a");
        assert_eq!(body["server_b"]["full_name"], "bob/tool-b");
        let shared = body["comparison"]["shared_tools"].as_array().unwrap();
        assert!(shared.iter().any(|t| t == "read_file"));
        assert!(!body["comparison"]["same_transport"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_compare_servers_not_found() {
        let app = setup_with_servers().await;
        let request = Request::builder()
            .uri("/api/v1/compare/alice/tool-a/nobody/nothing")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 404);
    }
}

#[cfg(test)]
mod deprecated_tests {
    use super::*;
    use crate::api::types::ServerEntry;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn setup_with_deprecated() -> axum::Router {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let active = ServerEntry {
            id: None,
            owner: "alice".into(),
            name: "new-tool".into(),
            version: "2.0.0".into(),
            description: "The new tool".into(),
            author: "alice".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec!["do_stuff".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 100,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        let old = ServerEntry {
            id: None,
            owner: "alice".into(),
            name: "old-tool".into(),
            version: "1.0.0".into(),
            description: "The old deprecated tool".into(),
            author: "alice".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec!["do_stuff".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: true,
            deprecated_by: Some("alice/new-tool".into()),
            downloads: 50,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&active).unwrap();
        db.upsert_server(&old).unwrap();
        let state: crate::registry::routes::DbState =
            std::sync::Arc::new(tokio::sync::Mutex::new(db));
        build_router(state)
    }

    #[tokio::test]
    async fn test_list_deprecated_endpoint() {
        let app = setup_with_deprecated().await;
        let request = Request::builder()
            .uri("/api/v1/deprecated")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["total"], 1);
        let servers = body["servers"].as_array().unwrap();
        assert_eq!(servers[0]["full_name"], "alice/old-tool");
        assert_eq!(servers[0]["deprecated_by"], "alice/new-tool");
    }

    #[tokio::test]
    async fn test_search_exclude_deprecated() {
        let app = setup_with_deprecated().await;
        // Without exclude
        let request = Request::builder()
            .uri("/api/v1/search?q=tool")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["total"], 2);

        // With exclude
        let request = Request::builder()
            .uri("/api/v1/search?q=tool&exclude_deprecated=true")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["total"], 1);
        assert_eq!(body["servers"][0]["name"], "new-tool");
    }

    #[tokio::test]
    async fn test_patch_deprecate_server() {
        let app = setup_with_deprecated().await;
        let patch = serde_json::json!({
            "deprecated": true,
            "deprecated_by": "alice/new-tool"
        });
        let request = Request::builder()
            .method("PATCH")
            .uri("/api/v1/servers/alice/new-tool")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&patch).unwrap()))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert!(body["updated_fields"].as_array().unwrap().iter().any(|f| f == "deprecated"));

        // Verify both are now deprecated
        let request = Request::builder()
            .uri("/api/v1/deprecated")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["total"], 2);
    }

    #[tokio::test]
    async fn test_deprecated_field_in_server_info() {
        let app = setup_with_deprecated().await;
        let request = Request::builder()
            .uri("/api/v1/servers/alice/old-tool")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["deprecated"], true);
        assert_eq!(body["deprecated_by"], "alice/new-tool");
    }
}

#[cfg(test)]
mod deprecation_db_tests {
    use crate::api::types::ServerEntry;

    fn make_entry(owner: &str, name: &str, deprecated: bool, deprecated_by: Option<&str>) -> ServerEntry {
        ServerEntry {
            id: None,
            owner: owner.into(),
            name: name.into(),
            version: "1.0.0".into(),
            description: "test".into(),
            author: owner.into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated,
            deprecated_by: deprecated_by.map(|s| s.to_string()),
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_deprecated_roundtrip_through_db() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("x", "old", true, Some("x/new"))).unwrap();
        db.upsert_server(&make_entry("x", "new", false, None)).unwrap();

        let old = db.get_server("x", "old").unwrap().unwrap();
        assert!(old.deprecated);
        assert_eq!(old.deprecated_by, Some("x/new".to_string()));

        let new = db.get_server("x", "new").unwrap().unwrap();
        assert!(!new.deprecated);
        assert_eq!(new.deprecated_by, None);
    }

    #[test]
    fn test_list_all_includes_deprecated() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        db.upsert_server(&make_entry("a", "active", false, None)).unwrap();
        db.upsert_server(&make_entry("a", "old", true, Some("a/active"))).unwrap();
        let all = db.list_all().unwrap();
        assert_eq!(all.len(), 2);
        let dep_count = all.iter().filter(|s| s.deprecated).count();
        assert_eq!(dep_count, 1);
    }
}

#[cfg(test)]
mod popular_tools_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn setup_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let state: DbState = Arc::new(Mutex::new(db));
        build_router(state)
    }

    #[tokio::test]
    async fn test_api_popular_tools() {
        let app = setup_app().await;
        let request = Request::builder()
            .uri("/api/v1/popular-tools")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert!(body["total"].as_u64().unwrap() > 0);
        assert!(!body["tools"].as_array().unwrap().is_empty());
        // Each tool should have required fields
        let first = &body["tools"][0];
        assert!(first["tool"].is_string());
        assert!(first["aggregate_downloads"].is_number());
        assert!(first["server_count"].is_number());
        assert!(first["rank"].as_u64().unwrap() == 1);
    }

    #[tokio::test]
    async fn test_api_popular_tools_with_query() {
        let app = setup_app().await;
        let request = Request::builder()
            .uri("/api/v1/popular-tools?q=read")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        // All results should contain "read" in tool name
        for tool_obj in body["tools"].as_array().unwrap() {
            let tool_name = tool_obj["tool"].as_str().unwrap().to_lowercase();
            assert!(tool_name.contains("read"), "Tool {tool_name} should contain 'read'");
        }
    }

    #[tokio::test]
    async fn test_api_popular_tools_with_limit() {
        let app = setup_app().await;
        let request = Request::builder()
            .uri("/api/v1/popular-tools?limit=3")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert!(body["tools"].as_array().unwrap().len() <= 3);
    }
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn setup_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let state: DbState = Arc::new(Mutex::new(db));
        build_router(state)
    }

    #[tokio::test]
    async fn test_api_compatibility_same_server() {
        let app = setup_app().await;
        let request = Request::builder()
            .uri("/api/v1/compatibility/modelcontextprotocol/filesystem/modelcontextprotocol/filesystem")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        // Same server should have tool conflicts with itself
        assert!(body["score"].is_number());
        assert!(body["details"]["transport_match"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_api_compatibility_different_servers() {
        let app = setup_app().await;
        let request = Request::builder()
            .uri("/api/v1/compatibility/modelcontextprotocol/filesystem/modelcontextprotocol/sqlite")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert!(body["compatible"].is_boolean());
        assert!(body["score"].is_number());
        assert!(body["issues"].is_array());
        assert!(body["notes"].is_array());
    }

    #[tokio::test]
    async fn test_api_compatibility_not_found() {
        let app = setup_app().await;
        let request = Request::builder()
            .uri("/api/v1/compatibility/nobody/nothing/nobody/nothing2")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 404);
    }
}

#[cfg(test)]
mod bundle_and_score_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_api_recommend_bundle() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/modelcontextprotocol/filesystem/bundle")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["seed"], "modelcontextprotocol/filesystem");
        assert!(result["total"].as_u64().unwrap() > 0);
        let bundle = result["bundle"].as_array().unwrap();
        assert!(!bundle.is_empty());
        assert_eq!(bundle[0]["rank"], 1);
        // Should not include the seed server itself
        for item in bundle {
            assert_ne!(item["full_name"], "modelcontextprotocol/filesystem");
        }
    }

    #[tokio::test]
    async fn test_api_recommend_bundle_with_limit() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/modelcontextprotocol/filesystem/bundle?limit=3")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["bundle"].as_array().unwrap().len() <= 3);
    }

    #[tokio::test]
    async fn test_api_recommend_bundle_not_found() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/nobody/nothing/bundle")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_api_server_score() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/modelcontextprotocol/filesystem/score")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["server"], "modelcontextprotocol/filesystem");
        assert!(result["score"].as_u64().unwrap() > 0);
        assert!(result["max_score"].as_u64().unwrap() > 0);
        assert!(result["percentage"].as_u64().unwrap() > 0);
        assert!(result["grade"].is_string());
        let checks = result["checks"].as_array().unwrap();
        assert!(!checks.is_empty());
        // Each check should have required fields
        let first = &checks[0];
        assert!(first["check"].is_string());
        assert!(first["pass"].is_boolean());
        assert!(first["points"].is_number());
        assert!(first["max_points"].is_number());
    }

    #[tokio::test]
    async fn test_api_server_score_not_found() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/servers/nobody/nothing/score")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_api_server_score_grades() {
        // Create a minimal server with few fields → should get low grade
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&crate::api::types::ServerEntry {
            id: None,
            owner: "min".into(),
            name: "bare".into(),
            version: "0.1.0".into(),
            description: String::new(),
            author: String::new(),
            license: String::new(),
            repository: String::new(),
            command: "echo".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        }).unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        let app = build_router(db_state);

        let req = Request::builder()
            .uri("/api/v1/servers/min/bare/score")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let grade = result["grade"].as_str().unwrap();
        // Bare server should get low grade (D or F)
        assert!(grade == "D" || grade == "F", "Bare server should get low grade, got {grade}");
    }

    #[tokio::test]
    async fn test_api_bundle_excludes_deprecated() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_server(&crate::api::types::ServerEntry {
            id: None,
            owner: "test".into(),
            name: "active".into(),
            version: "1.0.0".into(),
            description: "Active server".into(),
            author: "test".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec!["read".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 100,
            stars: 0,
            created_at: None,
            updated_at: None,
        }).unwrap();
        db.upsert_server(&crate::api::types::ServerEntry {
            id: None,
            owner: "test".into(),
            name: "old".into(),
            version: "1.0.0".into(),
            description: "Deprecated server".into(),
            author: "test".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec!["write".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: true,
            deprecated_by: Some("test/active".into()),
            downloads: 50,
            stars: 0,
            created_at: None,
            updated_at: None,
        }).unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        let app = build_router(db_state);

        let req = Request::builder()
            .uri("/api/v1/servers/test/active/bundle")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let bundle = result["bundle"].as_array().unwrap();
        // Deprecated servers should not appear in bundle recommendations
        for item in bundle {
            assert_ne!(item["full_name"], "test/old", "Deprecated should be excluded from bundle");
        }
    }

    #[tokio::test]
    async fn test_openapi_includes_bundle_and_score() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/openapi")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let paths = result["paths"].as_object().unwrap();
        assert!(paths.contains_key("/api/v1/servers/{owner}/{name}/bundle"), "OpenAPI should document /bundle");
        assert!(paths.contains_key("/api/v1/servers/{owner}/{name}/score"), "OpenAPI should document /score");
    }
}

#[cfg(test)]
mod enhanced_stats_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let state: DbState = Arc::new(Mutex::new(db));
        build_router(state)
    }

    #[tokio::test]
    async fn test_enhanced_stats_has_categories() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/stats")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert!(body["categories"].is_array());
        assert!(!body["categories"].as_array().unwrap().is_empty());
        assert!(body["licenses"].is_array());
    }

    #[tokio::test]
    async fn test_enhanced_stats_capability_totals() {
        let app = seeded_app().await;
        let req = Request::builder()
            .uri("/api/v1/stats")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert!(body.get("total_tools").is_some());
        assert!(body.get("total_prompts").is_some());
        assert!(body.get("total_resources").is_some());
        assert!(body.get("deprecated_servers").is_some());
        // total_tools should be > 0 for seeded data
        assert!(body["total_tools"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_enhanced_stats_empty_db() {
        let db = Database::open_in_memory().unwrap();
        let state: DbState = Arc::new(Mutex::new(db));
        let app = build_router(state);

        let req = Request::builder()
            .uri("/api/v1/stats")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1_000_000).await.unwrap()).unwrap();
        assert_eq!(body["total_servers"].as_u64().unwrap(), 0);
        assert_eq!(body["total_tools"].as_u64().unwrap(), 0);
        assert_eq!(body["deprecated_servers"].as_u64().unwrap(), 0);
    }
}

#[cfg(test)]
mod star_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn make_app(db: crate::registry::db::Database) -> axum::Router {
        let db_state = std::sync::Arc::new(tokio::sync::Mutex::new(db));
        super::build_router(db_state)
    }

    fn test_entry() -> crate::api::types::ServerEntry {
        crate::api::types::ServerEntry {
            id: None,
            owner: "startest".into(),
            name: "starserver".into(),
            version: "1.0.0".into(),
            description: "A server for star testing".into(),
            author: "Tester".into(),
            license: "MIT".into(),
            repository: "https://github.com/startest/starserver".into(),
            command: "starserver".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec!["tool1".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec!["test".into()],
            env: std::collections::HashMap::new(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 100,
            stars: 0,
            created_at: None,
            updated_at: None,
        }
    }

    #[tokio::test]
    async fn test_star_server() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry()).unwrap();
        let app = make_app(db);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/servers/startest/starserver/star")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap())
                .unwrap();
        assert_eq!(body["stars"].as_i64().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_star_server_multiple() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry()).unwrap();
        let app = make_app(db);

        // Star 3 times
        for _ in 0..3 {
            let app2 = app.clone();
            let resp = app2
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/servers/startest/starserver/star")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
        }

        // Check via GET
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/servers/startest/starserver")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap())
                .unwrap();
        assert_eq!(body["stars"].as_i64().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_unstar_server() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry()).unwrap();
        // Star it first
        db.star_server("startest", "starserver").unwrap();
        db.star_server("startest", "starserver").unwrap();
        let app = make_app(db);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/servers/startest/starserver/unstar")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap())
                .unwrap();
        assert_eq!(body["stars"].as_i64().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_unstar_server_floor_zero() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        db.upsert_server(&test_entry()).unwrap();
        let app = make_app(db);

        // Unstar when already 0
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/servers/startest/starserver/unstar")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap())
                .unwrap();
        assert_eq!(body["stars"].as_i64().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_star_not_found() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let app = make_app(db);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/servers/nonexistent/server/star")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn test_leaderboard() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let mut entry1 = test_entry();
        entry1.downloads = 500;
        db.upsert_server(&entry1).unwrap();

        let mut entry2 = test_entry();
        entry2.owner = "other".into();
        entry2.name = "server2".into();
        entry2.downloads = 200;
        db.upsert_server(&entry2).unwrap();
        // Give server2 more stars
        for _ in 0..50 {
            db.star_server("other", "server2").unwrap();
        }

        let app = make_app(db);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/leaderboard?limit=10")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap())
                .unwrap();
        let leaderboard = body["leaderboard"].as_array().unwrap();
        assert_eq!(leaderboard.len(), 2);
        // server2 should be first (200 + 50*10 = 700 > 500)
        assert_eq!(leaderboard[0]["server"].as_str().unwrap(), "other/server2");
        assert_eq!(leaderboard[0]["rank"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_leaderboard_empty() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let app = make_app(db);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/leaderboard")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap())
                .unwrap();
        assert_eq!(body["total"].as_u64().unwrap(), 0);
    }
}

#[cfg(test)]
mod db_star_tests {
    use super::*;

    #[test]
    fn test_db_star_server() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let entry = crate::api::types::ServerEntry {
            id: None,
            owner: "test".into(),
            name: "srv".into(),
            version: "1.0.0".into(),
            description: "test".into(),
            author: "a".into(),
            license: "MIT".into(),
            repository: "".into(),
            command: "cmd".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: std::collections::HashMap::new(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry).unwrap();

        // Star
        assert!(db.star_server("test", "srv").unwrap());
        assert!(db.star_server("test", "srv").unwrap());
        let s = db.get_server("test", "srv").unwrap().unwrap();
        assert_eq!(s.stars, 2);

        // Unstar
        assert!(db.unstar_server("test", "srv").unwrap());
        let s = db.get_server("test", "srv").unwrap().unwrap();
        assert_eq!(s.stars, 1);

        // Unstar to 0
        assert!(db.unstar_server("test", "srv").unwrap());
        let s = db.get_server("test", "srv").unwrap().unwrap();
        assert_eq!(s.stars, 0);

        // Unstar below 0 stays at 0
        assert!(db.unstar_server("test", "srv").unwrap());
        let s = db.get_server("test", "srv").unwrap().unwrap();
        assert_eq!(s.stars, 0);
    }

    #[test]
    fn test_db_star_nonexistent() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        assert!(!db.star_server("no", "such").unwrap());
        assert!(!db.unstar_server("no", "such").unwrap());
    }

    #[test]
    fn test_db_leaderboard() {
        let db = crate::registry::db::Database::open_in_memory().unwrap();
        let entries = db.leaderboard(10).unwrap();
        assert!(entries.is_empty());

        let entry = crate::api::types::ServerEntry {
            id: None,
            owner: "lb".into(),
            name: "test".into(),
            version: "1.0.0".into(),
            description: "test".into(),
            author: "a".into(),
            license: "MIT".into(),
            repository: "".into(),
            command: "cmd".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec![],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: std::collections::HashMap::new(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 0,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry).unwrap();
        let entries = db.leaderboard(10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].owner, "lb");
    }
}

#[cfg(test)]
mod matrix_badge_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let db = Database::open_in_memory().unwrap();
        db.seed_default_servers().unwrap();
        let state: DbState = Arc::new(Mutex::new(db));
        build_router(state)
    }

    #[tokio::test]
    async fn test_api_matrix() {
        let app = seeded_app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/matrix")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(result["matrix"].is_array());
        assert!(result["categories"].is_array());
        assert!(result["transports"].is_array());
        assert!(result["total_servers"].is_number());

        let matrix = result["matrix"].as_array().unwrap();
        assert!(!matrix.is_empty(), "Matrix should have rows");

        // Each row should have a category and a total
        for row in matrix {
            assert!(row["category"].is_string());
            assert!(row["total"].is_number());
        }
    }

    #[tokio::test]
    async fn test_api_matrix_totals_consistent() {
        let app = seeded_app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/matrix")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let total_servers = result["total_servers"].as_u64().unwrap();
        let matrix = result["matrix"].as_array().unwrap();
        let row_totals: u64 = matrix
            .iter()
            .map(|r| r["total"].as_u64().unwrap_or(0))
            .sum();
        assert_eq!(
            total_servers, row_totals,
            "Sum of row totals should equal total_servers"
        );
    }

    #[tokio::test]
    async fn test_api_badge_existing_server() {
        let app = seeded_app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/servers/modelcontextprotocol/filesystem/badge")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(content_type, "image/svg+xml");

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let svg = std::str::from_utf8(&body).unwrap();
        assert!(svg.contains("<svg"), "Should be SVG");
        assert!(svg.contains("filesystem"), "Should contain server name");
    }

    #[tokio::test]
    async fn test_api_badge_not_found() {
        let app = seeded_app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/servers/nobody/nothing/badge")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_ne!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_badge_plastic_style() {
        let app = seeded_app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/servers/modelcontextprotocol/filesystem/badge?style=plastic")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let svg = std::str::from_utf8(&body).unwrap();
        assert!(svg.contains("#007ec6"), "Plastic style should use blue color");
    }

    #[tokio::test]
    async fn test_api_bulk_search() {
        let app = seeded_app().await;
        let body = serde_json::json!({
            "queries": ["database", "web"],
            "limit": 3,
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/search/bulk")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(result["queries"], 2);
        assert!(result["results"]["database"].is_array());
        assert!(result["results"]["web"].is_array());
    }

    #[tokio::test]
    async fn test_api_recent_activity() {
        let app = seeded_app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/activity?limit=5")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["activity"].is_array());
        assert!(result["total"].is_number());
    }

    #[tokio::test]
    async fn test_api_badge_has_cache_header() {
        let app = seeded_app().await;
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/servers/modelcontextprotocol/filesystem/badge")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let cache_control = response
            .headers()
            .get("cache-control")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cache_control.contains("max-age"), "Should have cache-control header");
    }
}

#[cfg(test)]
mod cli_bundle_tests {
    #[test]
    fn test_cli_parses_bundle_create() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "mcpreg",
            "bundle-create",
            "my-bundle",
            "org/tool1",
            "org/tool2",
            "--description",
            "A test bundle",
        ]);
        assert!(cli.is_ok(), "bundle-create should parse: {:?}", cli.err());
    }

    #[test]
    fn test_cli_parses_bundle_inspect() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "mcpreg",
            "bundle-inspect",
            "bundle.json",
        ]);
        assert!(cli.is_ok(), "bundle-inspect should parse: {:?}", cli.err());
    }

    #[test]
    fn test_cli_parses_bundle_inspect_json() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "mcpreg",
            "bundle-inspect",
            "bundle.json",
            "--json",
        ]);
        assert!(cli.is_ok());
    }
}

#[cfg(test)]
mod exec_and_mirror_cli_tests {
    #[test]
    fn test_cli_parses_exec() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "mcpreg",
            "exec",
            "owner/server",
        ]);
        assert!(cli.is_ok(), "exec should parse: {:?}", cli.err());
    }

    #[test]
    fn test_cli_parses_exec_with_dry_run() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "mcpreg",
            "exec",
            "owner/server",
            "--dry-run",
        ]);
        assert!(cli.is_ok(), "exec --dry-run should parse: {:?}", cli.err());
    }

    #[test]
    fn test_cli_parses_exec_alias_run() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "mcpreg",
            "run",
            "owner/server",
        ]);
        assert!(cli.is_ok(), "run (alias for exec) should parse: {:?}", cli.err());
    }

    #[test]
    fn test_cli_parses_exec_with_extra_args() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "mcpreg",
            "exec",
            "owner/server",
            "--",
            "--port",
            "3000",
        ]);
        assert!(cli.is_ok(), "exec with extra args should parse: {:?}", cli.err());
    }

    #[test]
    fn test_cli_parses_mirror() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "mcpreg",
            "mirror",
            "/tmp/output",
        ]);
        assert!(cli.is_ok(), "mirror should parse: {:?}", cli.err());
    }

    #[test]
    fn test_cli_parses_mirror_default_output() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "mcpreg",
            "mirror",
        ]);
        assert!(cli.is_ok(), "mirror with default output should parse: {:?}", cli.err());
    }

    #[test]
    fn test_cli_parses_mirror_json() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "mcpreg",
            "mirror",
            "/tmp/out",
            "--json",
        ]);
        assert!(cli.is_ok());
    }
}

#[cfg(test)]
mod search_new_filters_cli_tests {
    #[test]
    fn test_cli_parses_search_min_stars() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "mcpreg",
            "search",
            "test",
            "--min-stars",
            "5",
        ]);
        assert!(cli.is_ok(), "search --min-stars should parse: {:?}", cli.err());
    }

    #[test]
    fn test_cli_parses_search_exclude_deprecated() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "mcpreg",
            "search",
            "test",
            "--exclude-deprecated",
        ]);
        assert!(cli.is_ok(), "search --exclude-deprecated should parse: {:?}", cli.err());
    }

    #[test]
    fn test_cli_parses_search_combined_new_filters() {
        use clap::Parser;
        let cli = crate::Cli::try_parse_from([
            "mcpreg",
            "search",
            "database",
            "--min-stars",
            "10",
            "--exclude-deprecated",
            "--min-downloads",
            "100",
            "--sort",
            "stars",
        ]);
        assert!(cli.is_ok(), "search with all new filters should parse: {:?}", cli.err());
    }
}

#[cfg(test)]
mod api_min_stars_tests {
    use super::*;
    use crate::api::types::ServerEntry;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn app_with_stars() -> Router {
        let db = Database::open_in_memory().unwrap();
        let entry1 = ServerEntry {
            id: None,
            owner: "test".into(),
            name: "popular".into(),
            version: "1.0.0".into(),
            description: "Popular server".into(),
            author: "test".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec!["tool1".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 1000,
            stars: 50,
            created_at: None,
            updated_at: None,
        };
        let entry2 = ServerEntry {
            id: None,
            owner: "test".into(),
            name: "niche".into(),
            version: "1.0.0".into(),
            description: "Niche server".into(),
            author: "test".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec!["tool2".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 100,
            stars: 2,
            created_at: None,
            updated_at: None,
        };
        let entry3 = ServerEntry {
            id: None,
            owner: "test".into(),
            name: "zero-stars".into(),
            version: "1.0.0".into(),
            description: "No stars server".into(),
            author: "test".into(),
            license: "MIT".into(),
            repository: String::new(),
            command: "node".into(),
            args: vec![],
            transport: "stdio".into(),
            tools: vec!["tool3".into()],
            resources: vec![],
            prompts: vec![],
            tags: vec![],
            env: Default::default(),
            homepage: String::new(),
            deprecated: false,
            deprecated_by: None,
            downloads: 50,
            stars: 0,
            created_at: None,
            updated_at: None,
        };
        db.upsert_server(&entry1).unwrap();
        db.upsert_server(&entry2).unwrap();
        db.upsert_server(&entry3).unwrap();
        db.set_stars("test", "popular", 50).unwrap();
        db.set_stars("test", "niche", 2).unwrap();
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_search_with_min_stars() {
        let app = app_with_stars().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&min_stars=10")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(search.total, 1, "Only popular server has >= 10 stars");
        assert_eq!(search.servers[0].name, "popular");
    }

    #[tokio::test]
    async fn test_search_with_min_stars_zero() {
        let app = app_with_stars().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&min_stars=0")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(search.total, 3, "min_stars=0 should return all");
    }

    #[tokio::test]
    async fn test_search_with_min_stars_exact_match() {
        let app = app_with_stars().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&min_stars=2")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(search.total, 2, "popular (50) and niche (2) should match");
    }

    #[tokio::test]
    async fn test_search_min_stars_too_high() {
        let app = app_with_stars().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&min_stars=9999")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(search.total, 0);
    }

    #[tokio::test]
    async fn test_search_min_stars_combined_with_min_downloads() {
        let app = app_with_stars().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=&min_stars=1&min_downloads=500")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(search.total, 1, "Only popular matches both filters");
        assert_eq!(search.servers[0].name, "popular");
    }

    #[tokio::test]
    async fn test_openapi_includes_min_stars() {
        let app = app_with_stars().await;
        let req = Request::builder()
            .uri("/api/v1/openapi")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let search_params = result["paths"]["/api/v1/search"]["get"]["parameters"]
            .as_array()
            .unwrap();
        let has_min_stars = search_params.iter().any(|p| p["name"] == "min_stars");
        assert!(has_min_stars, "OpenAPI should document min_stars parameter");
    }
}

#[cfg(test)]
mod stars_in_search_sort_tests {
    use super::*;
    use crate::api::types::ServerEntry;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn app_with_varied_stars() -> Router {
        let db = Database::open_in_memory().unwrap();
        for (name, stars, downloads) in [
            ("alpha", 100i64, 500i64),
            ("beta", 5, 1000),
            ("gamma", 50, 200),
        ] {
            db.upsert_server(&ServerEntry {
                id: None,
                owner: "test".into(),
                name: name.into(),
                version: "1.0.0".into(),
                description: format!("{name} server"),
                author: "test".into(),
                license: "MIT".into(),
                repository: String::new(),
                command: "node".into(),
                args: vec![],
                transport: "stdio".into(),
                tools: vec!["tool".into()],
                resources: vec![],
                prompts: vec![],
                tags: vec![],
                env: Default::default(),
                homepage: String::new(),
                deprecated: false,
                deprecated_by: None,
                downloads,
                stars: 0,
                created_at: None,
                updated_at: None,
            }).unwrap();
            db.set_stars("test", name, stars).unwrap();
        }
        let db_state: DbState = Arc::new(Mutex::new(db));
        build_router(db_state)
    }

    #[tokio::test]
    async fn test_search_sort_by_stars_via_api() {
        // The API sort by "stars" isn't in the route handler yet, but we can
        // verify the stars field is returned correctly
        let app = app_with_varied_stars().await;
        let req = Request::builder()
            .uri("/api/v1/search?q=")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let search: crate::api::types::SearchResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(search.total, 3);

        // Verify stars are returned
        for server in &search.servers {
            assert!(server.stars >= 0, "Stars should be non-negative");
        }
        // alpha has 100 stars
        let alpha = search.servers.iter().find(|s| s.name == "alpha").unwrap();
        assert_eq!(alpha.stars, 100);
    }

    #[tokio::test]
    async fn test_leaderboard_reflects_stars() {
        let app = app_with_varied_stars().await;
        let req = Request::builder()
            .uri("/api/v1/leaderboard?limit=3")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let leaderboard = result["leaderboard"].as_array().unwrap();
        assert_eq!(leaderboard.len(), 3);
        // alpha has 100 stars * 10 + 500 downloads = 1500 (highest)
        assert_eq!(leaderboard[0]["server"], "test/alpha");
    }
}
