// App ID Finder - main.rs
// Web server and API endpoints for iOS and Android app bundle ID lookup
//
// Copyright (c) 2025, 2026 Richard Vidal Dorsch
// SPDX-License-Identifier: MIT
// See LICENSE file in the project root for full license information.

use axum::{
    BoxError, Router,
    error_handling::HandleErrorLayer,
    extract::Query,
    http::{StatusCode, header},
    response::{Html, IntoResponse, Json},
    routing::{get, get_service},
};
use local_ip_address::list_afinet_netifas;
use serde::Deserialize;
use std::net::SocketAddr;
use tower::ServiceBuilder;

/// Longest search query we forward to the upstream stores.
const MAX_QUERY_CHARS: usize = 100;

/// Maximum number of search requests handled at the same time. Each search
/// makes two outbound requests, so this also caps upstream load. Requests
/// over the limit are rejected at once with HTTP 503 (they do not queue).
const MAX_CONCURRENT_SEARCHES: usize = 32;

pub mod android_appid;
pub mod http_client;
pub mod ios_appid;

#[derive(Deserialize)]
struct SearchParams {
    #[serde(default)]
    q: String,
}

#[axum::debug_handler]
async fn search_apps(Query(params): Query<SearchParams>) -> Json<serde_json::Value> {
    let query = params.q.trim();

    if query.is_empty() {
        return Json(serde_json::json!({
            "results": [],
            "count": 0,
            "errors": [],
            "query": "Please provide a search query"
        }));
    }

    if query.chars().count() > MAX_QUERY_CHARS {
        return Json(serde_json::json!({
            "results": [],
            "count": 0,
            "errors": [],
            "query": format!("Search query is too long (max {} characters)", MAX_QUERY_CHARS)
        }));
    }

    // Run iOS and Android lookups concurrently
    let (ios_result, android_result) = tokio::join!(
        ios_appid::search_ios_app(query),
        android_appid::find_app_id(query)
    );

    let mut results = Vec::new();
    // Short, user-facing messages. Error details stay in the server log.
    let mut errors: Vec<&str> = Vec::new();

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
            eprintln!("iOS search error for {:?}: {}", query, e);
            errors.push("App Store lookup failed");
        }
    }

    // Android processing
    match android_result {
        Ok(Some(app_info)) => {
            results.push(serde_json::json!({
                "name": app_info.name.as_deref().unwrap_or(query),
                "bundleId": app_info.id,
                "platform": "Android",
                "storeUrl": app_info.url
            }));
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("Android search error for {:?}: {}", query, e);
            errors.push("Play Store lookup failed");
        }
    }

    let message = if !results.is_empty() {
        format!("Search results for: {}", query)
    } else if errors.is_empty() {
        format!("No results found for: {}", query)
    } else {
        format!("Could not complete the search for: {}", query)
    };

    Json(serde_json::json!({
        "count": results.len(),
        "results": results,
        "errors": errors,
        "query": message
    }))
}

/// Response for a search rejected because the server is at its limit.
async fn search_overloaded(_: BoxError) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({
            "results": [],
            "count": 0,
            "errors": ["Server is busy"],
            "query": "Server is busy, please try again"
        })),
    )
}

/// GET route for `handler` that runs at most `MAX_CONCURRENT_SEARCHES` at once
/// and rejects the rest with `search_overloaded`.
fn with_search_limit<H, T>(handler: H) -> axum::routing::MethodRouter
where
    H: axum::handler::Handler<T, ()>,
    T: 'static,
{
    // Build the service once. Clones of it share one concurrency counter.
    get_service(
        ServiceBuilder::new()
            .layer(HandleErrorLayer::new(search_overloaded))
            .load_shed()
            .concurrency_limit(MAX_CONCURRENT_SEARCHES)
            .service(handler.with_state(())),
    )
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "App ID Finder",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn serve_styles() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../web/styles.css"),
    )
}

/// Resolves when the process gets Ctrl+C or (on Unix) SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            eprintln!("Failed to listen for Ctrl+C: {}", e);
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                eprintln!("Failed to listen for SIGTERM: {}", e);
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    println!("Shutting down...");
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}

/// Ports to try, in order. With `APPID_PORT` set, only that port is tried,
/// so a busy port is an error instead of a silent move to another port.
/// Without it: 3000, then 3001, then a random free port (0).
fn candidate_ports(env_port: Option<&str>) -> Result<Vec<u16>, String> {
    match env_port.map(str::trim) {
        None | Some("") => Ok(vec![3000, 3001, 0]),
        Some(v) => v
            .parse::<u16>()
            .map(|p| vec![p])
            .map_err(|_| format!("APPID_PORT={:?} is not a valid port (0-65535)", v)),
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/api/search", with_search_limit(search_apps))
        .route("/api/health", get(health_check))
        .route("/", get(serve_index))
        .route("/web/styles.css", get(serve_styles));

    let ports = match candidate_ports(std::env::var("APPID_PORT").ok().as_deref()) {
        Ok(ports) => ports,
        Err(msg) => {
            eprintln!("Error: {}", msg);
            std::process::exit(1);
        }
    };
    let port_was_set = ports.len() == 1;

    let mut listener = None;
    for p in ports {
        let addr = SocketAddr::from(([0, 0, 0, 0], p));
        match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => {
                listener = Some(l);
                break;
            }
            Err(e) if port_was_set => {
                eprintln!("Error: cannot use APPID_PORT={}: {}", p, e);
                eprintln!("Stop the program that uses this port, or set a different APPID_PORT.");
                std::process::exit(1);
            }
            Err(e) => eprintln!("Port {} unavailable: {}", p, e),
        }
    }
    let Some(listener) = listener else {
        eprintln!("Error: could not bind any port");
        std::process::exit(1);
    };

    let port = listener.local_addr().unwrap().port();
    println!(
        "🚀 App ID Finder server listening on all interfaces, port {}",
        port
    );
    println!("   http://localhost:{}", port);
    println!("Endpoints: /api/search?q=app | /api/health | /web/styles.css");
    println!("🌐 Network access URLs (if reachable on your LAN):");
    if let Ok(ifaces) = list_afinet_netifas() {
        for (_name, ip) in ifaces {
            if ip.is_ipv4() && !ip.is_loopback() {
                println!("   http://{}:{}", ip, port);
            }
        }
    }
    if !port_was_set {
        println!("(Set APPID_PORT to choose a specific port.)");
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use std::time::Duration;
    use tower::ServiceExt;

    async fn slow_handler() -> &'static str {
        tokio::time::sleep(Duration::from_millis(300)).await;
        "ok"
    }

    #[test]
    fn test_candidate_ports() {
        assert_eq!(candidate_ports(None), Ok(vec![3000, 3001, 0]));
        assert_eq!(candidate_ports(Some("")), Ok(vec![3000, 3001, 0]));
        assert_eq!(candidate_ports(Some(" 8080 ")), Ok(vec![8080]));
        assert!(candidate_ports(Some("80a")).is_err());
        assert!(candidate_ports(Some("70000")).is_err());
    }

    #[tokio::test]
    async fn test_search_limit_rejects_overflow_with_503() {
        let app = Router::new().route("/s", with_search_limit(slow_handler));
        let extra = 5;
        let handles: Vec<_> = (0..MAX_CONCURRENT_SEARCHES + extra)
            .map(|_| {
                let app = app.clone();
                tokio::spawn(async move {
                    let req = Request::get("/s").body(Body::empty()).unwrap();
                    app.oneshot(req).await.unwrap().status()
                })
            })
            .collect();

        let mut ok = 0;
        let mut busy = 0;
        for h in handles {
            match h.await.unwrap() {
                StatusCode::OK => ok += 1,
                StatusCode::SERVICE_UNAVAILABLE => busy += 1,
                other => panic!("unexpected status {other}"),
            }
        }
        assert_eq!(ok, MAX_CONCURRENT_SEARCHES);
        assert_eq!(busy, extra);
    }
}
