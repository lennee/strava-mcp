use crate::models::StravaActivity;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json;

const BASE_URL: &str = "https://www.strava.com/api/v3";

pub struct StravaClient {
    client: Client,
}

impl StravaClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent("strava-mcp/1.0")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client })
    }

    /// Fetch activities from Strava API
    ///
    /// Parameters:
    /// - access_token: OAuth access token
    /// - after: Optional Unix timestamp to filter activities after this time
    /// - before: Optional Unix timestamp to filter activities before this time
    pub async fn fetch_activities(
        &self,
        access_token: &str,
        after: Option<i64>,
        before: Option<i64>,
    ) -> Result<Vec<StravaActivity>> {
        let url = format!("{}/athlete/activities", BASE_URL);

        // Add query parameters
        let mut params = vec![("per_page", "100".to_string())];

        if let Some(after_time) = after {
            params.push(("after", after_time.to_string()));
        }

        if let Some(before_time) = before {
            params.push(("before", before_time.to_string()));
        }

        let response = self
            .client
            .get(&url)
            .query(&params)
            .bearer_auth(access_token)
            .send()
            .await
            .context("Failed to send request to Strava API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Strava API error ({}): {}", status, body);
        }

        // Get the response text first for better error diagnostics
        let response_text = response
            .text()
            .await
            .context("Failed to read response body")?;

        // Try to parse the JSON
        let activities = serde_json::from_str::<Vec<StravaActivity>>(&response_text)
            .map_err(|e| {
                // Log the full response for debugging
                eprintln!("Parse error: {}", e);
                eprintln!("Full response body: {}", &response_text);
                anyhow::anyhow!(
                    "Failed to parse Strava API response. Parse error: {}. Response body (first 1000 chars): {}",
                    e,
                    &response_text[..response_text.len().min(1000)]
                )
            })?;

        Ok(activities)
    }
}

impl Default for StravaClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default StravaClient")
    }
}
