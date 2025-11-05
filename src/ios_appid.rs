// A library to fetch iOS app information from the iTunes API.
//
// Copyright (c) 2025 Richard Vidal Dorsch
// SPDX-License-Identifier: MIT
// See LICENSE file in the project root for full license information.

use serde::Deserialize;

/// Represents the top-level structure of the iTunes Search API JSON response.
/// We only care about the `results` field, so we ignore the rest.
#[derive(Deserialize, Debug)]
struct ApiResponse {
    results: Vec<SearchAppInfo>,
}

/// Represents an application's metadata within the `results` array for search.
/// We use `#[serde(rename_all = "camelCase")]` to automatically convert
/// the JSON's `bundleId` field to Rust's idiomatic `bundle_id`.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SearchAppInfo {
    bundle_id: String,
}

/// Represents the top-level structure of the iTunes Lookup API JSON response.
/// We only care about the `results` field, so we ignore the rest.
#[derive(Deserialize, Debug)]
struct LookupResponse {
    results: Vec<LookupAppInfo>,
}

/// Represents an application's metadata within the `results` array for lookup.
/// We use `#[serde(rename_all = "camelCase")]` to automatically convert
/// the JSON's camelCase keys (like trackId) to Rust's snake_case (track_id).
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct LookupAppInfo {
    track_id: u64,
}

/// Fetches the bundle ID for a given iOS application name.
///
/// This function is asynchronous and returns a `Result` containing either the
/// bundle ID as a `String` or an error.
///
/// # Arguments
///
/// * `app_name` - A string slice that holds the name of the app to search for.
///
/// # Errors
///
/// This function will return an error if:
/// - The HTTP request fails.
/// - The response body cannot be parsed into the expected JSON structure.
/// - The app is not found and the API returns no results.
///
/// # Example
///
/// ```rust
/// use ios_appid::get_bundle_id;
///
/// #[tokio::main]
/// async fn main() {
///     match get_bundle_id("youtube").await {
///         Ok(id) => assert_eq!(id, "com.google.ios.youtube"),
///         Err(e) => panic!("Failed to get bundle ID: {}", e),
///     }
/// }
/// ```
pub async fn get_bundle_id(app_name: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // 1. Construct the URL with query parameters.
    // This is safer than string formatting as it handles URL encoding for the app_name.
    let url = reqwest::Url::parse_with_params(
        "https://itunes.apple.com/search",
        &[
            ("term", app_name),
            ("entity", "software"),
            ("country", "us"),
            ("limit", "1"),
        ],
    )?;

    // 2. Perform the asynchronous GET request.
    let response = reqwest::get(url)
        .await?
        .json::<ApiResponse>() // 3. Attempt to deserialize the JSON body into our ApiResponse struct.
        .await?;

    // 4. Extract the first result. If no results are found, return a clear error.
    // `into_iter().next()` consumes the Vec and efficiently gets the first element.
    if let Some(app_info) = response.results.into_iter().next() {
        Ok(app_info.bundle_id)
    } else {
        Err(format!("No app found with the name '{}'", app_name).into())
    }
}

/// Fetches the Apple App Store URL for a given iOS bundle ID.
///
/// # Arguments
///
/// * `bundle_id` - A string slice that holds the bundle ID of the iOS app (e.g., "com.hulu.plus").
///
/// # Returns
///
/// A `Result` which is:
/// - `Ok(String)` containing the full App Store URL if successful.
/// - `Err(Box<dyn std::error::Error>)` if the request fails, JSON parsing fails, or the app is not found.
///
/// # Example
///
/// ```rust
/// use ios_appid::get_app_store_url;
///
/// #[tokio::main]
/// async fn main() {
///     match get_app_store_url("com.google.ios.youtube").await {
///         Ok(url) => println!("App Store URL: {}", url),
///         Err(e) => panic!("Failed to get App Store URL: {}", e),
///     }
/// }
/// ```
pub async fn get_app_store_url(bundle_id: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // 1. Construct the API URL with proper URL encoding using reqwest::Url::parse_with_params
    let url = reqwest::Url::parse_with_params(
        "https://itunes.apple.com/lookup",
        &[("bundleId", bundle_id)],
    )?;

    // 2. Perform the asynchronous HTTP GET request.
    // The '?' operator will propagate any errors from the request.
    let response = reqwest::get(url).await?;

    // 3. Check if the HTTP request was successful.
    if !response.status().is_success() {
        return Err(format!("Request failed with status: {}", response.status()).into());
    }

    // 4. Parse the JSON response body into our defined structs.
    // The '?' will propagate parsing errors.
    let lookup_data: LookupResponse = response.json().await?;

    // 5. Extract the trackId from the first result.
    // The `results` array might be empty if the app isn't found.
    if let Some(app_info) = lookup_data.results.first() {
        // 6. Construct the final App Store URL and return it.
        let app_store_url = format!("https://apps.apple.com/app/id{}", app_info.track_id);
        Ok(app_store_url)
    } else {
        // Return an error if no app was found for the given bundle ID.
        Err(format!("No app found for bundle ID: {}", bundle_id).into())
    }
}

/// Fetches the Apple App Store URL for a given app name by combining bundle ID lookup and URL generation.
///
/// This function first finds the bundle ID for the given app name, then uses that bundle ID
/// to get the App Store URL.
///
/// # Arguments
///
/// * `app_name` - A string slice that holds the name of the app to search for (e.g., "YouTube").
///
/// # Returns
///
/// A `Result` which is:
/// - `Ok(String)` containing the full App Store URL if successful.
/// - `Err(Box<dyn std::error::Error>)` if any step fails (app not found, bundle ID lookup fails, etc.).
///
/// # Example
///
/// ```rust
/// use ios_appid::get_app_store_url_by_name;
///
/// #[tokio::main]
/// async fn main() {
///     match get_app_store_url_by_name("YouTube").await {
///         Ok(url) => println!("App Store URL: {}", url),
///         Err(e) => panic!("Failed to get App Store URL: {}", e),
///     }
/// }
/// ```
pub async fn get_app_store_url_by_name(
    app_name: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // First, get the bundle ID from the app name
    let bundle_id = get_bundle_id(app_name).await?;

    // Then, get the App Store URL from the bundle ID
    let app_store_url = get_app_store_url(&bundle_id).await?;

    Ok(app_store_url)
}

// Unit tests for our library function.
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_youtube_success() {
        let result = get_bundle_id("youtube").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "com.google.ios.youtube");
    }

    #[tokio::test]
    async fn test_fetch_instagram_success() {
        let result = get_bundle_id("instagram").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "com.burbn.instagram");
    }

    #[tokio::test]
    async fn test_app_not_found() {
        let app_name = "jhksfhjhaf";
        let result = get_bundle_id(app_name).await;
        assert!(result.is_err());
        // Check if the error message is what we expect.
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("No app found with the name '{}'", app_name)
        );
    }

    #[tokio::test]
    async fn test_get_app_store_url_success() {
        let bundle_id = "com.google.ios.youtube";
        let result = get_app_store_url(bundle_id).await;
        assert!(result.is_ok());
        let url = result.unwrap();
        assert!(url.starts_with("https://apps.apple.com/app/id"));
        assert!(url.contains("544007664")); // YouTube's track ID
    }

    #[tokio::test]
    async fn test_get_app_store_url_not_found() {
        let bundle_id = "com.nonexistent.app.test";
        let result = get_app_store_url(bundle_id).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("No app found for bundle ID: {}", bundle_id)
        );
    }

    #[tokio::test]
    async fn test_get_app_store_url_by_name_success() {
        let app_name = "youtube";
        let result = get_app_store_url_by_name(app_name).await;
        assert!(result.is_ok());
        let url = result.unwrap();
        assert!(url.starts_with("https://apps.apple.com/app/id"));
        assert!(url.contains("544007664")); // YouTube's track ID
    }

    #[tokio::test]
    async fn test_get_app_store_url_by_name_not_found() {
        let app_name = "nonexistentappxyz123test";
        let result = get_app_store_url_by_name(app_name).await;
        assert!(result.is_err());
        // Should fail at the bundle ID lookup step
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No app found with the name")
        );
    }
}
