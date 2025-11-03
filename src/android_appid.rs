// Copyright (c) 2025 Richard Vidal Dorsch
//
// This software is released under the MIT License.
// https://opensource.org/licenses/MIT

use reqwest;
use scraper::{Html, Selector};
use url::Url;
use urlencoding;

/// A struct to hold both the extracted URL and the App ID.
#[derive(Debug)]
pub struct AppInfo {
    pub url: String,
    pub id: String,
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
    let selector = match Selector::parse(r#"a[href*="/store/apps/details?id="]"#) {
        Ok(s) => s,
        Err(_) => return None,
    };

    // Find the first matching element (the primary app listing)
    if let Some(element) = document.select(&selector).next() {
        // Extract the 'href' attribute value (the relative URL path)
        if let Some(url_path) = element.attr("href") {
            // Reconstruct the full absolute URL for proper parsing
            let base_url = "https://play.google.com";
            let full_url = if url_path.starts_with(base_url) {
                url_path.to_string()
            } else {
                format!("{}{}", base_url, url_path)
            };

            // Use the 'url' crate to parse the URL and extract the 'id' parameter
            if let Ok(url) = Url::parse(&full_url) {
                for (key, value) in url.query_pairs() {
                    if key == "id" {
                        // Found the App ID, return both the full URL and the ID
                        return Some(AppInfo {
                            url: full_url,
                            id: value.into_owned(),
                        });
                    }
                }
            } else {
                eprintln!("Error: Could not parse URL retrieved from search result.");
            }
        }
    }

    // Return None if the link, App ID, or URL parsing failed
    None
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
pub async fn find_app_id(app_name: &str) -> Result<Option<AppInfo>, Box<dyn std::error::Error + Send + Sync>> {
    // Construct the Google Play Store search URL
    let search_query = urlencoding::encode(app_name);
    let request_url = format!(
        "https://play.google.com/store/search?q={}&c=apps",
        search_query
    );

    // Fetch the HTML content from the URL
    let response = reqwest::get(&request_url).await?;
    if !response.status().is_success() {
        // Handle non-successful HTTP status codes
        eprintln!(
            "Error: Request failed with status code {}",
            response.status()
        );
        return Ok(None);
    }

    // Extract the HTML content as text
    let html_content = response.text().await?;

    // Parse the HTML to find the app information
    Ok(extract_app_info_from_search(&html_content))
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
