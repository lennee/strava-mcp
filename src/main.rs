mod models;
mod oauth;
mod strava_api;
mod utils;

use anyhow::{Context, Result};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use models::OAuthConfig;
use oauth::TokenManager;
use rmcp::{
    handler::server::tool::ToolRouter,
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;
use strava_api::StravaClient;
use tokio::io::{stdin, stdout};
use utils::{format_distance, format_duration, format_pace};

#[derive(Clone)]
struct StravaMcpServer {
    token_manager: Arc<TokenManager>,
    strava_client: Arc<StravaClient>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl StravaMcpServer {
    fn new(token_manager: TokenManager, strava_client: StravaClient) -> Self {
        Self {
            token_manager: Arc::new(token_manager),
            strava_client: Arc::new(strava_client),
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
        let access_token = match &params.access_token {
            Some(token) => token.clone(),
            None => self
                .token_manager
                .get_valid_access_token()
                .await
                .map_err(McpError::internal)?,
        };

        // Parse date
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| McpError::invalid_params_no_data(format!("Invalid date format: {}", e)))?;

        // Calculate day boundaries in UTC
        let start_of_day = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| McpError::internal("Invalid date"))?
            .and_utc()
            .timestamp();
        let end_of_day = start_of_day + 86400; // 24 hours

        // Fetch activities
        let activities = self
            .strava_client
            .fetch_activities(&access_token, Some(start_of_day), Some(end_of_day))
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
        let mut total_time = 0u32;

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
            output.push_str(&format!(
                "- **Pace:** {}/km\n",
                format_pace(run.average_speed)
            ));
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
        let limit = params.limit.unwrap_or(5);
        let access_token = match &params.access_token {
            Some(token) => token.clone(),
            None => self
                .token_manager
                .get_valid_access_token()
                .await
                .map_err(McpError::internal)?,
        };

        // Fetch activities
        let activities = self
            .strava_client
            .fetch_activities(&access_token, None, None)
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
            output.push_str(&format!(
                "- **Pace:** {}/km\n",
                format_pace(run.average_speed)
            ));

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
        let access_token = match &params.access_token {
            Some(token) => token.clone(),
            None => self
                .token_manager
                .get_valid_access_token()
                .await
                .map_err(McpError::internal)?,
        };

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
        let activities = self
            .strava_client
            .fetch_activities(
                &access_token,
                Some(week_start_timestamp),
                Some(week_end_timestamp),
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
        let total_time: u32 = runs.iter().map(|r| r.moving_time).sum();
        let total_elevation: f64 = runs.iter().map(|r| r.total_elevation_gain).sum();

        // Calculate average pace
        let avg_pace = if total_time > 0 {
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
        let port = params.port.unwrap_or(8089);
        let scope = params.scope.as_deref().unwrap_or("activity:read_all");

        let result = self
            .token_manager
            .authorize(port, scope)
            .await
            .map_err(McpError::internal)?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
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
    #[schemars(
        description = "Optional Strava access token (uses environment token if not provided)"
    )]
    access_token: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetRecentRunsParams {
    #[schemars(description = "Number of recent runs to retrieve (default: 5)")]
    limit: Option<usize>,
    #[schemars(
        description = "Optional Strava access token (uses environment token if not provided)"
    )]
    access_token: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetWeeklySummaryParams {
    #[schemars(description = "Start of week in YYYY-MM-DD format (defaults to current Monday)")]
    week_start: Option<String>,
    #[schemars(
        description = "Optional Strava access token (uses environment token if not provided)"
    )]
    access_token: Option<String>,
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

    // Initialize token manager
    let token_manager = TokenManager::new(config).context("Failed to initialize token manager")?;

    // Initialize Strava client
    let strava_client = StravaClient::new().context("Failed to initialize Strava client")?;

    // Create MCP server
    let server = StravaMcpServer::new(token_manager, strava_client);

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
