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
            "/api/v1/servers/:owner/:name",
            axum::routing::get(routes::get_server)
                .delete(routes::delete_server),
        )
        .route(
            "/api/v1/servers/:owner/:name/download",
            axum::routing::post(routes::track_download),
        )
        .route(
            "/api/v1/servers/:owner/:name/similar",
            axum::routing::get(routes::similar_servers),
        )
        .route("/api/v1/servers", axum::routing::get(routes::list_servers))
        .route("/api/v1/servers/batch", axum::routing::post(routes::batch_get_servers))
        .route("/api/v1/publish", axum::routing::post(routes::publish))
        .route("/api/v1/stats", axum::routing::get(routes::stats))
        .route("/api/v1/tools", axum::routing::get(routes::tools_index))
        .route("/api/v1/categories", axum::routing::get(routes::categories))
        .route("/api/v1/prompts", axum::routing::get(routes::prompts_index))
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
            downloads: 1500,
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
            downloads: 0,
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
            downloads: 0,
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
            downloads: 0,
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
            downloads: 0,
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
            downloads: 0,
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
            downloads: 0,
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
