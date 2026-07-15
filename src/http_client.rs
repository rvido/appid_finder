// App ID Finder - http_client.rs
// Shared HTTP client configured with timeout and user-agent.
//
// Copyright (c) 2025, 2026 Richard Vidal Dorsch
// SPDX-License-Identifier: MIT

use std::sync::OnceLock;
use std::time::Duration;

/// Returns a reference to a shared, lazy-initialized `reqwest::Client`.
///
/// Configured with:
/// - A standard browser User-Agent to prevent bot-blocking.
/// - A connection timeout of 5 seconds.
pub fn get_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default()
    })
}
