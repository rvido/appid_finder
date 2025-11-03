use axum::{extract::Query, response::{Html, Json}, routing::get, Router};
use std::collections::HashMap;
use tower_http::{cors::CorsLayer, services::ServeDir};

pub mod android_appid;
pub mod ios_appid;

#[axum::debug_handler]
async fn search_apps(Query(params): Query<HashMap<String, String>>) -> Json<serde_json::Value> {
    let empty = String::new();
    let query = params.get("q").unwrap_or(&empty).trim();

    if query.is_empty() {
        return Json(serde_json::json!({
            "results": [],
            "count": 0,
            "query": "Please provide a search query"
        }));
    }

    // Run iOS and Android lookups concurrently
    let (ios_store_url_result, android_result) = tokio::join!(
        ios_appid::get_app_store_url_by_name(query),
        android_appid::find_app_id(query)
    );

    let mut results = Vec::new();

    // iOS processing: only add if we can also resolve bundle_id
    if let Ok(store_url) = ios_store_url_result {
        if let Ok(bundle_id) = ios_appid::get_bundle_id(query).await {
            results.push(serde_json::json!({
                "name": query,
                "bundleId": bundle_id,
                "platform": "iOS",
                "storeUrl": store_url
            }));
        }
    }

    // Android processing
    if let Ok(Some(app_info)) = android_result {
        results.push(serde_json::json!({
            "name": query,
            "bundleId": app_info.id,
            "platform": "Android",
            "storeUrl": app_info.url
        }));
    }

    let response = if results.is_empty() {
        serde_json::json!({
            "results": [],
            "count": 0,
            "query": format!("No results found for: {}", query)
        })
    } else {
        serde_json::json!({
            "results": results,
            "count": results.len(),
            "query": format!("Search results for: {}", query)
        })
    };

    Json(response)
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "App ID Finder",
        "version": "0.1.0"
    }))
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}

#[tokio::main]
async fn main() {
    let cors = CorsLayer::permissive();
    let app = Router::new()
        .route("/api/search", get(search_apps))
        .route("/api/health", get(health_check))
        .route("/", get(serve_index))
        .nest_service("/web", ServeDir::new("web"))
        .layer(cors);

    println!("🚀 App ID Finder server running at http://localhost:3000");
    println!("Endpoints: /api/search?q=app | /api/health | /web/styles.css");

    axum::serve(
        tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap(),
        app,
    )
    .await
    .unwrap();
}