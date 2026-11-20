// App ID Finder - ios_appid.rs
// iOS App Store API integration for bundle ID lookup
//
// Copyright (c) 2025, 2026 Richard Vidal Dorsch
// SPDX-License-Identifier: MIT
// See LICENSE file in the project root for full license information.

use serde::Deserialize;

/// Top-level structure of the iTunes Search API JSON response.
/// We only care about the `results` field, so we ignore the rest.
#[derive(Deserialize, Debug)]
struct ApiResponse {
    results: Vec<SearchAppInfo>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SearchAppInfo {
    bundle_id: String,
    track_id: u64,
    track_name: String,
}

/// Search result information for an iOS application.
#[derive(Debug, Clone)]
pub struct IosAppInfo {
    pub name: String,
    pub bundle_id: String,
    pub store_url: String,
}

/// Searches for an iOS app by name and retrieves its metadata in a single request.
///
/// Returns `Ok(None)` when the store has no match, and `Err` when the
/// request fails, the store returns an error status, or the body is invalid.
pub async fn search_ios_app(
    app_name: &str,
) -> Result<Option<IosAppInfo>, Box<dyn std::error::Error + Send + Sync>> {
    let url = reqwest::Url::parse_with_params(
        "https://itunes.apple.com/search",
        &[
            ("term", app_name),
            ("entity", "software"),
            ("country", "us"),
            ("limit", "1"),
        ],
    )?;

    let response = crate::http_client::get_client().get(url).send().await?;
    let body = crate::http_client::read_body_capped(response).await?;
    let parsed: ApiResponse = serde_json::from_slice(&body)?;

    Ok(parsed.results.into_iter().next().map(|app| IosAppInfo {
        store_url: format!("https://apps.apple.com/app/id{}", app.track_id),
        name: app.track_name,
        bundle_id: app.bundle_id,
    }))
}

// Live network tests: they call the real iTunes API.
// Run them with `cargo test -- --ignored`.
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "live network test"]
    async fn test_search_youtube() {
        let info = search_ios_app("youtube").await.unwrap().unwrap();
        assert_eq!(info.bundle_id, "com.google.ios.youtube");
        assert_eq!(info.store_url, "https://apps.apple.com/app/id544007664");
    }

    #[tokio::test]
    #[ignore = "live network test"]
    async fn test_search_instagram() {
        let info = search_ios_app("instagram").await.unwrap().unwrap();
        assert_eq!(info.bundle_id, "com.burbn.instagram");
    }

    #[tokio::test]
    #[ignore = "live network test"]
    async fn test_app_not_found() {
        let result = search_ios_app("nonexistentappxyz123test").await.unwrap();
        assert!(result.is_none());
    }
}
