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
    pub total_elevation_gain: f64,
    pub average_speed: f64, // meters per second
    pub max_speed: f64,     // meters per second
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_heartrate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_heartrate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffer_score: Option<i32>,
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
            average_speed: 2.77,
            max_speed: 3.5,
            average_heartrate: Some(150.0),
            max_heartrate: Some(180.0),
            suffer_score: Some(50),
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
            ..run
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
