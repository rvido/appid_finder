// App ID Finder - android_appid.rs
// Google Play Store scraping for Android app package names
//
// Copyright (c) 2025, 2026 Richard Vidal Dorsch
// SPDX-License-Identifier: MIT
// See LICENSE file in the project root for full license information.

use scraper::{Html, Selector};
use url::Url;

/// A struct to hold both the extracted URL and the App ID.
#[derive(Debug)]
pub struct AppInfo {
    pub url: String,
    pub id: String,
}

/// Maximum size of a Play Store search page we are willing to buffer.
const MAX_BODY_BYTES: usize = 5 * 1024 * 1024;

/// Returns true if `id` looks like a valid Android package name
/// (e.g. "com.spotify.music"). This rejects anything that could be
/// used for HTML/URL injection further down the line.
fn is_valid_package_name(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 255
        && id.contains('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

/// Extracts the first app's detail URL and its App ID from the
/// Google Play Store search results HTML.
///
/// Returns: An Option<AppInfo> containing the URL and App ID if found,
/// or None otherwise.
fn extract_app_info_from_search(html_content: &str) -> Option<AppInfo> {
    // Parse the HTML content
    let document = Html::parse_document(html_content);

    // Define the CSS selector to find the app detail link
    // Targets any <a> tag whose 'href' attribute contains the unique identifier for
    // an app details page: "/store/apps/details?id=".
    let selector = Selector::parse(r#"a[href*="/store/apps/details?id="]"#).ok()?;
    let base_url = Url::parse("https://play.google.com").ok()?;

    // Find the first matching element (the primary app listing)
    let url_path = document.select(&selector).next()?.attr("href")?;

    // Resolve the (usually relative) href against the Play Store origin and
    // extract the 'id' parameter.
    let url = match base_url.join(url_path) {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Error: Could not parse URL retrieved from search result.");
            return None;
        }
    };
    let id = url
        .query_pairs()
        .find(|(key, _)| key == "id")
        .map(|(_, value)| value.into_owned())?;

    if !is_valid_package_name(&id) {
        return None;
    }

    // Build the store URL ourselves instead of trusting the scraped href,
    // so it always points at play.google.com.
    Some(AppInfo {
        url: format!("https://play.google.com/store/apps/details?id={}", id),
        id,
    })
}

/// Searches for an app on the Google Play Store and returns its information.
///
/// # Arguments
///
/// * `app_name` - The name of the app to search for.
///
/// # Returns
///
/// A `Result` containing an `Option<AppInfo>` if the operation was successful,
/// or an error otherwise.
pub async fn find_app_id(
    app_name: &str,
) -> Result<Option<AppInfo>, Box<dyn std::error::Error + Send + Sync>> {
    // Construct the Google Play Store search URL
    let search_query = urlencoding::encode(app_name);
    let request_url = format!(
        "https://play.google.com/store/search?q={}&c=apps",
        search_query
    );

    // Fetch the HTML content from the URL using the shared HTTP client.
    let response = crate::http_client::get_client()
        .get(&request_url)
        .send()
        .await?;
    if !response.status().is_success() {
        // Handle non-successful HTTP status codes
        eprintln!(
            "Error: Request failed with status code {}",
            response.status()
        );
        return Ok(None);
    }

    // Read the body with a size cap so a huge response cannot exhaust memory.
    let mut response = response;
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > MAX_BODY_BYTES {
            return Err("Play Store response too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    let html_content = String::from_utf8_lossy(&body).into_owned();

    // HTML parsing is CPU-bound; run it off the async worker threads.
    let app_info =
        tokio::task::spawn_blocking(move || extract_app_info_from_search(&html_content)).await?;
    Ok(app_info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_app_info_from_search_spotify() {
        let html_content = r#"
            <a href="/store/apps/details?id=com.spotify.music"></a>
        "#;
        let app_info = extract_app_info_from_search(html_content);
        assert!(app_info.is_some());
        let info = app_info.unwrap();
        assert_eq!(info.id, "com.spotify.music");
        assert_eq!(
            info.url,
            "https://play.google.com/store/apps/details?id=com.spotify.music"
        );
    }

    #[test]
    fn test_extract_app_info_rejects_injected_id() {
        let html_content = r#"
            <a href="/store/apps/details?id=%3Cimg%20src%3Dx%20onerror%3Dalert(1)%3E"></a>
        "#;
        assert!(extract_app_info_from_search(html_content).is_none());
    }

    #[test]
    fn test_extract_app_info_ignores_foreign_host() {
        let html_content = r#"
            <a href="https://evil.example/store/apps/details?id=com.spotify.music"></a>
        "#;
        let info = extract_app_info_from_search(html_content).unwrap();
        assert_eq!(
            info.url,
            "https://play.google.com/store/apps/details?id=com.spotify.music"
        );
    }

    #[test]
    fn test_extract_app_info_no_link() {
        let html_content = "<div>No link here</div>";
        let app_info = extract_app_info_from_search(html_content);
        assert!(app_info.is_none());
    }

    #[tokio::test]
    async fn test_find_spotify_app_id() {
        let app_name = "spotify";
        let result = find_app_id(app_name).await;

        assert!(result.is_ok());
        let app_info = result.unwrap();
        assert!(app_info.is_some());
        let info = app_info.unwrap();
        assert_eq!(info.id, "com.spotify.music");
    }

    #[tokio::test]
    async fn test_app_not_found() {
        let app_name = "some_non_existent_app_12345";
        let result = find_app_id(app_name).await;

        assert!(result.is_ok());
        let app_info = result.unwrap();
        assert!(app_info.is_none());
    }
}
