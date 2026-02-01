use crate::models::StravaActivity;
use anyhow::{Context, Result};
use reqwest::Client;

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

        let activities = response
            .json::<Vec<StravaActivity>>()
            .await
            .context("Failed to parse Strava API response")?;

        Ok(activities)
    }
}

impl Default for StravaClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default StravaClient")
    }
}
