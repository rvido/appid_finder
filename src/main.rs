// App ID Finder - main.rs
// Web server and API endpoints for iOS and Android app bundle ID lookup
//
// Copyright (c) 2025, 2026 Richard Vidal Dorsch
// SPDX-License-Identifier: MIT
// See LICENSE file in the project root for full license information.

use axum::{
    Router,
    extract::Query,
    response::{Html, Json},
    routing::get,
};
use local_ip_address::list_afinet_netifas;
use std::collections::HashMap;
use std::net::SocketAddr;
use tower_http::{cors::CorsLayer, services::ServeDir};

pub mod android_appid;
pub mod http_client;
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
    let (ios_result, android_result) = tokio::join!(
        ios_appid::search_ios_app(query),
        android_appid::find_app_id(query)
    );

    let mut results = Vec::new();

    // iOS processing: retrieve name, bundle ID, and store URL in a single step
    match ios_result {
        Ok(Some(app_info)) => {
            results.push(serde_json::json!({
                "name": app_info.name,
                "bundleId": app_info.bundle_id,
                "platform": "iOS",
                "storeUrl": app_info.store_url
            }));
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("iOS search error for '{}': {}", query, e);
        }
    }

    // Android processing
    match android_result {
        Ok(Some(app_info)) => {
            results.push(serde_json::json!({
                "name": query,
                "bundleId": app_info.id,
                "platform": "Android",
                "storeUrl": app_info.url
            }));
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("Android search error for '{}': {}", query, e);
        }
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
    let configured_port: u16 = std::env::var("APPID_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);
    let mut candidate_ports = vec![configured_port];
    if configured_port != 3001 {
        candidate_ports.push(3001);
    }
    // ephemeral fallback
    candidate_ports.push(0);

    let listener = {
        let mut bound = None;
        for p in candidate_ports {
            let addr: SocketAddr = format!("0.0.0.0:{}", p).parse().unwrap();
            match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => {
                    bound = Some(l);
                    break;
                }
                Err(e) => {
                    eprintln!("Port {} unavailable: {}", p, e);
                    continue;
                }
            }
        }
        bound.expect("Failed to bind any candidate port")
    };

    let actual_addr = listener.local_addr().unwrap();
    println!(
        "🚀 App ID Finder server listening on http://{}",
        actual_addr
    );
    println!("Endpoints: /api/search?q=app | /api/health | /web/styles.css");
    println!("🌐 Network access URLs (if reachable on your LAN):");
    if let Ok(ifaces) = list_afinet_netifas() {
        for (_name, ip) in ifaces {
            if ip.is_ipv4() && !ip.is_loopback() {
                println!("   http://{}:{}", ip, actual_addr.port());
            }
        }
    }
    println!(
        "(Set APPID_PORT to choose a specific port. Chose {}.)",
        actual_addr.port()
    );

    axum::serve(listener, app).await.unwrap();
}
