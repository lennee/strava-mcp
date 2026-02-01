use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Strava activity data from the API
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StravaActivity {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub activity_type: String,
    pub sport_type: String,
    pub start_date: String,
    pub start_date_local: String,
    pub distance: f64,     // meters
    pub moving_time: u32,  // seconds
    pub elapsed_time: u32, // seconds
    #[serde(default)]
    pub total_elevation_gain: f64, // meters
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average_speed: Option<f64>, // meters per second
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_speed: Option<f64>,     // meters per second
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average_heartrate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_heartrate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suffer_score: Option<f64>,
    // Additional fields that may be present in the API response - all optional
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub athlete: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_state: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utc_offset: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_city: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub achievement_count: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kudos_count: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment_count: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub athlete_count: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo_count: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commute: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flagged: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average_cadence: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average_watts: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_watts: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weighted_average_watts: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kilojoules: Option<f64>,
}

impl StravaActivity {
    /// Check if this activity is a run
    pub fn is_run(&self) -> bool {
        self.activity_type == "Run" || self.sport_type == "Run" || self.sport_type == "TrailRun"
    }
}

/// Cached OAuth token with expiration time
#[derive(Debug, Clone)]
pub struct TokenCache {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64, // Unix timestamp
}

impl TokenCache {
    /// Check if the token is expiring within the given buffer (in seconds)
    pub fn is_expiring_soon(&self, buffer_seconds: i64) -> bool {
        let now = Utc::now().timestamp();
        now + buffer_seconds >= self.expires_at
    }

    /// Check if the token is already expired
    #[allow(dead_code)]
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp();
        now >= self.expires_at
    }
}

/// OAuth configuration from environment variables
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
}

impl OAuthConfig {
    /// Load OAuth config from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        let client_id = std::env::var("STRAVA_CLIENT_ID")
            .map_err(|_| anyhow::anyhow!("STRAVA_CLIENT_ID environment variable not set"))?;
        let client_secret = std::env::var("STRAVA_CLIENT_SECRET")
            .map_err(|_| anyhow::anyhow!("STRAVA_CLIENT_SECRET environment variable not set"))?;

        Ok(Self {
            client_id,
            client_secret,
        })
    }
}

/// OAuth token response from Strava
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_run() {
        let run = StravaActivity {
            id: 1,
            name: "Morning Run".to_string(),
            activity_type: "Run".to_string(),
            sport_type: "Run".to_string(),
            start_date: "2024-01-01T10:00:00Z".to_string(),
            start_date_local: "2024-01-01T10:00:00Z".to_string(),
            distance: 5000.0,
            moving_time: 1800,
            elapsed_time: 1900,
            total_elevation_gain: 50.0,
            average_speed: Some(2.77),
            max_speed: Some(3.5),
            average_heartrate: Some(150.0),
            max_heartrate: Some(180.0),
            suffer_score: Some(50.0),
            resource_state: Some(2),
            athlete: None,
            device_name: None,
            timezone: None,
            utc_offset: None,
            location_city: None,
            location_state: None,
            location_country: None,
            achievement_count: Some(0),
            kudos_count: Some(0),
            comment_count: Some(0),
            athlete_count: Some(1.0),
            photo_count: Some(0),
            trainer: Some(false),
            commute: Some(false),
            manual: Some(false),
            private: Some(false),
            flagged: Some(false),
            average_cadence: None,
            average_watts: None,
            max_watts: None,
            weighted_average_watts: None,
            kilojoules: None,
        };
        assert!(run.is_run());

        let trail_run = StravaActivity {
            sport_type: "TrailRun".to_string(),
            ..run.clone()
        };
        assert!(trail_run.is_run());

        let ride = StravaActivity {
            activity_type: "Ride".to_string(),
            sport_type: "MountainBikeRide".to_string(),
            ..run.clone()
        };
        assert!(!ride.is_run());
    }

    #[test]
    fn test_token_expiration() {
        let now = Utc::now().timestamp();

        // Token expires in 10 minutes
        let token = TokenCache {
            access_token: "test".to_string(),
            refresh_token: "refresh".to_string(),
            expires_at: now + 600,
        };

        // Not expired
        assert!(!token.is_expired());

        // Not expiring soon (within 5 minutes buffer - token expires in 10 minutes)
        assert!(!token.is_expiring_soon(300));

        // Expiring soon (within 15 minutes buffer - token expires in 10 minutes)
        assert!(token.is_expiring_soon(900));
    }
}
