use crate::registry::db::Database;
use crate::registry::routes::{self, DbState};
use axum::Router;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

pub async fn run_server(bind_addr: &str, db_path: &str) -> crate::error::Result<()> {
    let db = Database::open(db_path)?;
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
        .route("/api/v1/search", axum::routing::get(routes::search))
        .route(
            "/api/v1/servers/:owner/:name",
            axum::routing::get(routes::get_server),
        )
        .route("/api/v1/servers", axum::routing::get(routes::list_servers))
        .route("/api/v1/publish", axum::routing::post(routes::publish))
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
        // Seed test data
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
}
