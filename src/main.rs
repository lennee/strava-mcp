mod utils;

use anyhow::{Context, Result};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use rmcp::{
    handler::server::tool::ToolRouter,
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;
use strava_api::{AuthenticatedClient, OAuthConfig, TokenStorage};
use tokio::io::{stdin, stdout};
use utils::{format_distance, format_duration, format_pace};

// Helper trait for checking if an activity is a run
trait ActivityExt {
    fn is_run(&self) -> bool;
}

impl ActivityExt for strava_api::SummaryActivity {
    fn is_run(&self) -> bool {
        self.activity_type == "Run" || self.sport_type == "Run" || self.sport_type == "TrailRun"
    }
}

#[derive(Clone)]
struct StravaMcpServer {
    auth_client: Arc<AuthenticatedClient>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl StravaMcpServer {
    fn new(auth_client: AuthenticatedClient) -> Self {
        Self {
            auth_client: Arc::new(auth_client),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get running activities for a specific date (YYYY-MM-DD format)")]
    async fn get_runs_for_date(
        &self,
        params: rmcp::handler::server::wrapper::Parameters<GetRunsForDateParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = params.0; // Extract inner value
        let date_str = &params.date;

        // Validate date string length (prevent excessive parsing)
        if date_str.len() > 10 {
            return Err(McpError::invalid_params_no_data(
                "Date must be in YYYY-MM-DD format (10 characters max)",
            ));
        }

        // Get authenticated client (with auto token refresh)
        let client = self.auth_client.client().await.map_err(McpError::internal)?;

        // Parse and validate date
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| McpError::invalid_params_no_data(format!("Invalid date format (expected YYYY-MM-DD): {}", e)))?;

        // Validate date is within reasonable range (Strava founded in 2009)
        let min_date = NaiveDate::from_ymd_opt(2009, 1, 1)
            .ok_or_else(|| McpError::internal("Failed to create min date"))?;
        let max_date = Utc::now().date_naive() + Duration::days(1); // Allow today + 1 day for timezone differences

        if date < min_date {
            return Err(McpError::invalid_params_no_data(format!(
                "Date {} is before Strava existed (min: 2009-01-01)",
                date
            )));
        }

        if date > max_date {
            return Err(McpError::invalid_params_no_data(format!(
                "Date {} is in the future (max: {})",
                date, max_date
            )));
        }

        // Calculate day boundaries in UTC
        let start_of_day = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| McpError::internal("Invalid date"))?
            .and_utc()
            .timestamp();
        let end_of_day = start_of_day + 86400; // 24 hours

        // Fetch activities
        let activities = client
            .list_athlete_activities(Some(start_of_day), Some(end_of_day), 1, 200)
            .await
            .map_err(McpError::internal)?;

        // Filter for runs
        let runs: Vec<_> = activities.iter().filter(|a| a.is_run()).collect();

        if runs.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "No runs found for {}",
                date_str
            ))]));
        }

        // Format output
        let mut output = format!("# Runs for {}\n\n", date_str);

        let mut total_distance = 0.0;
        let mut total_time = 0i32;

        for run in &runs {
            output.push_str(&format!("## {}\n", run.name));
            output.push_str(&format!(
                "- **Distance:** {} km\n",
                format_distance(run.distance)
            ));
            output.push_str(&format!(
                "- **Duration:** {}\n",
                format_duration(run.moving_time)
            ));
            if let Some(avg_speed) = run.average_speed {
                output.push_str(&format!(
                    "- **Pace:** {}/km\n",
                    format_pace(avg_speed)
                ));
            }
            output.push_str(&format!(
                "- **Elevation Gain:** {:.0}m\n",
                run.total_elevation_gain
            ));

            if let Some(hr) = run.average_heartrate {
                output.push_str(&format!("- **Average Heart Rate:** {:.0} bpm\n", hr));
            }
            if let Some(max_hr) = run.max_heartrate {
                output.push_str(&format!("- **Max Heart Rate:** {:.0} bpm\n", max_hr));
            }

            output.push('\n');

            total_distance += run.distance;
            total_time += run.moving_time;
        }

        // Add totals if multiple runs
        if runs.len() > 1 {
            output.push_str("## Totals\n");
            output.push_str(&format!("- **Runs:** {}\n", runs.len()));
            output.push_str(&format!(
                "- **Total Distance:** {} km\n",
                format_distance(total_distance)
            ));
            output.push_str(&format!(
                "- **Total Time:** {}\n",
                format_duration(total_time)
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Get the most recent running activities")]
    async fn get_recent_runs(
        &self,
        params: rmcp::handler::server::wrapper::Parameters<GetRecentRunsParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = params.0;

        // Validate and bound the limit parameter (prevent DoS)
        const MAX_LIMIT: usize = 100;
        const DEFAULT_LIMIT: usize = 5;
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT);

        if limit == 0 {
            return Err(McpError::invalid_params_no_data("limit must be greater than 0"));
        }

        if limit > MAX_LIMIT {
            return Err(McpError::invalid_params_no_data(format!(
                "limit cannot exceed {} (requested: {})",
                MAX_LIMIT, limit
            )));
        }

        // Get authenticated client (with auto token refresh)
        let client = self.auth_client.client().await.map_err(McpError::internal)?;

        // Fetch activities
        let activities = client
            .list_athlete_activities(None, None, 1, 200)
            .await
            .map_err(McpError::internal)?;

        // Filter for runs and take limit
        let runs: Vec<_> = activities
            .iter()
            .filter(|a| a.is_run())
            .take(limit)
            .collect();

        if runs.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "No recent runs found".to_string(),
            )]));
        }

        // Format output
        let mut output = format!("# {} Most Recent Runs\n\n", runs.len());

        for run in runs {
            // Parse local date for display
            let date = run.start_date_local.split('T').next().unwrap_or("Unknown");

            output.push_str(&format!("## {} ({})\n", run.name, date));
            output.push_str(&format!(
                "- **Distance:** {} km\n",
                format_distance(run.distance)
            ));
            output.push_str(&format!(
                "- **Duration:** {}\n",
                format_duration(run.moving_time)
            ));
            if let Some(avg_speed) = run.average_speed {
                output.push_str(&format!(
                    "- **Pace:** {}/km\n",
                    format_pace(avg_speed)
                ));
            }

            if let Some(hr) = run.average_heartrate {
                output.push_str(&format!("- **Average HR:** {:.0} bpm\n", hr));
            }

            output.push('\n');
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Get weekly running summary (defaults to current week)")]
    async fn get_weekly_summary(
        &self,
        params: rmcp::handler::server::wrapper::Parameters<GetWeeklySummaryParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = params.0;

        // Get authenticated client (with auto token refresh)
        let client = self.auth_client.client().await.map_err(McpError::internal)?;

        // Determine week start (Monday)
        let week_start = match &params.week_start {
            Some(date_str) => NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|e| {
                McpError::invalid_params_no_data(format!("Invalid date format: {}", e))
            })?,
            None => {
                let today = Utc::now().date_naive();
                // Find the previous Monday
                let days_since_monday = today.weekday().num_days_from_monday();
                today - Duration::days(days_since_monday as i64)
            }
        };

        // Calculate week boundaries
        let week_start_timestamp = week_start
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| McpError::internal("Invalid date"))?
            .and_utc()
            .timestamp();
        let week_end_timestamp = week_start_timestamp + (7 * 86400); // 7 days

        // Fetch activities
        let activities = client
            .list_athlete_activities(
                Some(week_start_timestamp),
                Some(week_end_timestamp),
                1,
                200,
            )
            .await
            .map_err(McpError::internal)?;

        // Filter for runs
        let runs: Vec<_> = activities.iter().filter(|a| a.is_run()).collect();

        if runs.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "No runs found for week starting {}",
                week_start
            ))]));
        }

        // Calculate aggregates
        let total_runs = runs.len();
        let total_distance: f64 = runs.iter().map(|r| r.distance).sum();
        let total_time: i32 = runs.iter().map(|r| r.moving_time).sum();
        let total_elevation: f64 = runs.iter().map(|r| r.total_elevation_gain).sum();

        // Calculate average pace from total distance and time
        let avg_pace = if total_time > 0 && total_distance > 0.0 {
            total_distance / total_time as f64
        } else {
            0.0
        };

        // Format output
        let week_end = week_start + Duration::days(6);
        let mut output = format!("# Weekly Summary: {} to {}\n\n", week_start, week_end);

        output.push_str(&format!("- **Total Runs:** {}\n", total_runs));
        output.push_str(&format!(
            "- **Total Distance:** {} km\n",
            format_distance(total_distance)
        ));
        output.push_str(&format!(
            "- **Total Time:** {}\n",
            format_duration(total_time)
        ));
        output.push_str(&format!(
            "- **Average Pace:** {}/km\n",
            format_pace(avg_pace)
        ));
        output.push_str(&format!(
            "- **Total Elevation Gain:** {:.0}m\n",
            total_elevation
        ));

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = "Authorize the MCP with your Strava account")]
    async fn authorize(
        &self,
        params: rmcp::handler::server::wrapper::Parameters<AuthorizeParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = params.0;

        // Validate port parameter (prevent privilege escalation)
        const MIN_PORT: u16 = 1024; // Avoid privileged ports
        const MAX_PORT: u16 = 65535;
        const DEFAULT_PORT: u16 = 8089;

        let port = params.port.unwrap_or(DEFAULT_PORT);

        if port < MIN_PORT {
            return Err(McpError::invalid_params_no_data(format!(
                "port must be >= {} (requested: {}). Ports below 1024 require elevated privileges.",
                MIN_PORT, port
            )));
        }

        if port > MAX_PORT {
            return Err(McpError::invalid_params_no_data(format!(
                "port must be <= {} (requested: {})",
                MAX_PORT, port
            )));
        }

        // Validate scope parameter (whitelist allowed scopes)
        const ALLOWED_SCOPES: &[&str] = &[
            "read",
            "read_all",
            "profile:read_all",
            "profile:write",
            "activity:read",
            "activity:read_all",
            "activity:write",
        ];

        let scope = params.scope.as_deref().unwrap_or("activity:read_all");

        if !ALLOWED_SCOPES.contains(&scope) {
            return Err(McpError::invalid_params_no_data(format!(
                "Invalid scope '{}'. Allowed scopes: {}",
                scope,
                ALLOWED_SCOPES.join(", ")
            )));
        }

        // Authorize and get access token
        self.auth_client
            .authorize(port, scope)
            .await
            .map_err(McpError::internal)?;

        // Save token for persistence
        let storage = TokenStorage::default_location().map_err(McpError::internal)?;
        if let Some(token) = self.auth_client.get_token().await {
            storage.save(&token).map_err(McpError::internal)?;
        }

        Ok(CallToolResult::success(vec![Content::text(
            "Authorization successful! Token saved for future use.".to_string(),
        )]))
    }
}

#[tool_handler]
impl ServerHandler for StravaMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("MCP server for Strava API integration. Provides tools to fetch and analyze running activity data from Strava.".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

// Tool parameter structs
#[derive(Debug, Deserialize, JsonSchema)]
struct GetRunsForDateParams {
    #[schemars(description = "Date in YYYY-MM-DD format")]
    date: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetRecentRunsParams {
    #[schemars(description = "Number of recent runs to retrieve (default: 5)")]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetWeeklySummaryParams {
    #[schemars(description = "Start of week in YYYY-MM-DD format (defaults to current Monday)")]
    week_start: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AuthorizeParams {
    #[schemars(description = "Port for OAuth callback server (default: 8089)")]
    port: Option<u16>,
    #[schemars(description = "OAuth scope (default: 'activity:read_all')")]
    scope: Option<String>,
}

// Helper methods for McpError
trait McpErrorExt {
    fn internal<E: std::fmt::Display>(error: E) -> Self;
    fn invalid_params_no_data<S: Into<String>>(message: S) -> Self;
}

impl McpErrorExt for McpError {
    fn internal<E: std::fmt::Display>(error: E) -> Self {
        McpError::internal_error(format!("Internal error: {}", error), None)
    }

    fn invalid_params_no_data<S: Into<String>>(message: S) -> Self {
        McpError::invalid_params(message.into(), None)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Load OAuth config
    let config = OAuthConfig::from_env()
        .context("Failed to load OAuth configuration. Please set STRAVA_CLIENT_ID and STRAVA_CLIENT_SECRET environment variables.")?;

    // Load or create authenticated client with token persistence
    let storage = TokenStorage::default_location()
        .context("Failed to get token storage location")?;

    let auth_client = if storage.exists() {
        // Load existing token from storage
        let token = storage.load()
            .context("Failed to load saved token")?;
        eprintln!("Loaded saved authentication token");
        AuthenticatedClient::with_token(config, token)
    } else {
        // No saved token, will need to authorize on first tool call
        eprintln!("No saved token found. Use the 'authorize' tool to authenticate.");
        AuthenticatedClient::new(config)
    };

    // Create MCP server
    let server = StravaMcpServer::new(auth_client);

    // Create stdio transport
    let transport = (stdin(), stdout());

    // Serve
    eprintln!("Starting Strava MCP server...");
    let service = server.serve(transport).await.map_err(|e| {
        eprintln!("Error starting server: {}", e);
        e
    })?;

    service.waiting().await?;

    Ok(())
}
