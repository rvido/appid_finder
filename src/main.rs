use axum::{extract::Query, response::{Html, Json}, routing::get, Router};
use std::collections::HashMap;
use std::net::SocketAddr;
use local_ip_address::list_afinet_netifas;
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

    // Preferred port via env APPID_PORT else 3000; provide fallbacks if occupied.
    let configured_port: u16 = std::env::var("APPID_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(3000);
    let mut candidate_ports = vec![configured_port];
    if configured_port != 3001 { candidate_ports.push(3001); }
    // ephemeral fallback
    candidate_ports.push(0);

    let listener = {
        let mut bound = None;
        for p in candidate_ports {
            let addr: SocketAddr = format!("0.0.0.0:{}", p).parse().unwrap();
            match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => { bound = Some(l); break; },
                Err(e) => {
                    eprintln!("Port {} unavailable: {}", p, e);
                    continue;
                }
            }
        }
        bound.expect("Failed to bind any candidate port")
    };

    let actual_addr = listener.local_addr().unwrap();
    println!("🚀 App ID Finder server listening on http://{}", actual_addr);
    println!("Endpoints: /api/search?q=app | /api/health | /web/styles.css");
    println!("🌐 Network access URLs (if reachable on your LAN):");
    if let Ok(ifaces) = list_afinet_netifas() {
        for (_name, ip) in ifaces {
            if ip.is_ipv4() && !ip.is_loopback() {
                println!("   http://{}:{}", ip, actual_addr.port());
            }
        }
    }
    println!("(Set APPID_PORT to choose a specific port. Chose {}.)", actual_addr.port());

    axum::serve(listener, app).await.unwrap();
}