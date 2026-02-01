use crate::models::{OAuthConfig, TokenCache, TokenResponse};
use anyhow::{Context, Result};
use axum::{
    extract::Query,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use reqwest::Client;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

const TOKEN_REFRESH_BUFFER: i64 = 300; // 5 minutes in seconds
const AUTH_URL: &str = "https://www.strava.com/oauth/authorize";
const TOKEN_URL: &str = "https://www.strava.com/api/v3/oauth/token";

/// Token manager for handling OAuth tokens
pub struct TokenManager {
    cache: Arc<Mutex<Option<TokenCache>>>,
    config: OAuthConfig,
    http_client: Client,
}

impl TokenManager {
    /// Create a new token manager
    pub fn new(config: OAuthConfig) -> Result<Self> {
        let http_client = Client::new();

        // Try to load tokens from environment
        let initial_cache = Self::load_tokens_from_env_sync().ok();
        let has_tokens = initial_cache.is_some();
        let cache = Arc::new(Mutex::new(initial_cache));

        if !has_tokens {
            eprintln!("Warning: Could not load tokens from environment");
        }

        Ok(Self {
            cache,
            config,
            http_client,
        })
    }

    /// Load tokens from environment variables (synchronous)
    fn load_tokens_from_env_sync() -> Result<TokenCache> {
        let access_token =
            std::env::var("STRAVA_ACCESS_TOKEN").context("STRAVA_ACCESS_TOKEN not found")?;
        let refresh_token =
            std::env::var("STRAVA_REFRESH_TOKEN").context("STRAVA_REFRESH_TOKEN not found")?;
        let expires_at = std::env::var("STRAVA_EXPIRES_AT")
            .context("STRAVA_EXPIRES_AT not found")?
            .parse::<i64>()
            .context("STRAVA_EXPIRES_AT is not a valid number")?;

        Ok(TokenCache {
            access_token,
            refresh_token,
            expires_at,
        })
    }

    /// Save tokens to .env file
    fn save_tokens_to_env(&self, token: &TokenCache) -> Result<()> {
        let env_path = Path::new(".env");
        let mut content = if env_path.exists() {
            fs::read_to_string(env_path).context("Failed to read .env file")?
        } else {
            String::new()
        };

        // Update or append tokens
        let token_vars = [
            ("STRAVA_ACCESS_TOKEN", &token.access_token),
            ("STRAVA_REFRESH_TOKEN", &token.refresh_token),
            ("STRAVA_EXPIRES_AT", &token.expires_at.to_string()),
        ];

        for (key, value) in &token_vars {
            let pattern = format!("{}=", key);
            let new_line = format!("{}={}\n", key, value);

            if content.contains(&pattern) {
                // Replace existing line
                let lines: Vec<String> = content
                    .lines()
                    .map(|line| {
                        if line.starts_with(&pattern) {
                            format!("{}={}", key, value)
                        } else {
                            line.to_string()
                        }
                    })
                    .collect();
                content = lines.join("\n") + "\n";
            } else {
                // Append new line
                if !content.is_empty() && !content.ends_with('\n') {
                    content.push('\n');
                }
                content.push_str(&new_line);
            }
        }

        fs::write(env_path, content).context("Failed to write .env file")?;

        // Update environment variables
        std::env::set_var("STRAVA_ACCESS_TOKEN", &token.access_token);
        std::env::set_var("STRAVA_REFRESH_TOKEN", &token.refresh_token);
        std::env::set_var("STRAVA_EXPIRES_AT", token.expires_at.to_string());

        Ok(())
    }

    /// Get a valid access token, refreshing if necessary
    pub async fn get_valid_access_token(&self) -> Result<String> {
        let cache_guard = self.cache.lock().await;

        match &*cache_guard {
            Some(token) => {
                if token.is_expiring_soon(TOKEN_REFRESH_BUFFER) {
                    drop(cache_guard); // Release lock before async operation
                    self.refresh_access_token().await
                } else {
                    Ok(token.access_token.clone())
                }
            }
            None => {
                drop(cache_guard);
                anyhow::bail!("No access token available. Please run the authorize tool first.")
            }
        }
    }

    /// Refresh the access token using the refresh token
    async fn refresh_access_token(&self) -> Result<String> {
        let refresh_token = {
            let cache_guard = self.cache.lock().await;
            match &*cache_guard {
                Some(token) => token.refresh_token.clone(),
                None => anyhow::bail!("No refresh token available"),
            }
        };

        let params = [
            ("client_id", self.config.client_id.as_str()),
            ("client_secret", self.config.client_secret.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
        ];

        let response = self
            .http_client
            .post(TOKEN_URL)
            .form(&params)
            .send()
            .await
            .context("Failed to refresh access token")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to refresh token ({}): {}", status, body);
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .context("Failed to parse token response")?;

        let new_token = TokenCache {
            access_token: token_response.access_token.clone(),
            refresh_token: token_response.refresh_token,
            expires_at: token_response.expires_at,
        };

        // Update cache
        *self.cache.lock().await = Some(new_token.clone());

        // Save to .env
        if let Err(e) = self.save_tokens_to_env(&new_token) {
            eprintln!("Warning: Failed to save tokens to .env: {}", e);
        }

        Ok(new_token.access_token)
    }

    /// Start OAuth authorization flow
    pub async fn authorize(&self, port: u16, scope: &str) -> Result<String> {
        // Build authorization URL
        let mut auth_url = Url::parse(AUTH_URL)?;
        auth_url
            .query_pairs_mut()
            .append_pair("client_id", &self.config.client_id)
            .append_pair(
                "redirect_uri",
                &format!("http://localhost:{}/callback", port),
            )
            .append_pair("response_type", "code")
            .append_pair("scope", scope)
            .append_pair("approval_prompt", "auto");

        // Open browser
        println!("Opening browser for authorization...");
        if let Err(e) = open::that(auth_url.as_str()) {
            eprintln!(
                "Failed to open browser: {}. Please open this URL manually:",
                e
            );
            println!("{}", auth_url);
        }

        // Create shared state for communication
        let callback_result: Arc<Mutex<Option<Result<String>>>> = Arc::new(Mutex::new(None));
        let callback_result_clone = callback_result.clone();

        // Create the OAuth callback server
        let app = Router::new().route(
            "/callback",
            get(move |query: Query<CallbackParams>| {
                callback_handler(query, callback_result_clone.clone())
            }),
        );

        // Start server
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
            .await
            .context("Failed to bind to port")?;

        println!(
            "Waiting for authorization callback on http://localhost:{}...",
            port
        );

        // Spawn server task
        let server_handle = tokio::spawn(async move { axum::serve(listener, app).await });

        // Wait for callback with timeout
        let start = tokio::time::Instant::now();
        let timeout_duration = tokio::time::Duration::from_secs(120);
        let code: String = loop {
            if start.elapsed() >= timeout_duration {
                anyhow::bail!("Authorization timeout after 2 minutes");
            }

            if let Some(result) = callback_result.lock().await.take() {
                break result?;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        };

        // Exchange code for tokens
        let token = self.exchange_code_for_tokens(&code).await?;

        // Cache the token
        *self.cache.lock().await = Some(token.clone());

        // Save to .env
        if let Err(e) = self.save_tokens_to_env(&token) {
            eprintln!("Warning: Failed to save tokens to .env: {}", e);
        }

        // Server will be automatically dropped/closed
        server_handle.abort();

        Ok("Authorization successful! Tokens have been saved.".to_string())
    }

    /// Exchange authorization code for access tokens
    async fn exchange_code_for_tokens(&self, code: &str) -> Result<TokenCache> {
        let params = [
            ("client_id", self.config.client_id.as_str()),
            ("client_secret", self.config.client_secret.as_str()),
            ("code", code),
            ("grant_type", "authorization_code"),
        ];

        let response = self
            .http_client
            .post(TOKEN_URL)
            .form(&params)
            .send()
            .await
            .context("Failed to exchange code for tokens")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to exchange code ({}): {}", status, body);
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .context("Failed to parse token response")?;

        Ok(TokenCache {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            expires_at: token_response.expires_at,
        })
    }
}

#[derive(Debug, Deserialize)]
struct CallbackParams {
    code: Option<String>,
    error: Option<String>,
}

async fn callback_handler(
    Query(params): Query<CallbackParams>,
    result: Arc<Mutex<Option<Result<String>>>>,
) -> impl IntoResponse {
    if let Some(error) = params.error {
        *result.lock().await = Some(Err(anyhow::anyhow!("Authorization error: {}", error)));
        return Html(format!(
            r#"
            <html>
                <body>
                    <h1>Authorization Failed</h1>
                    <p>Error: {}</p>
                    <p>You can close this window.</p>
                </body>
            </html>
            "#,
            error
        ));
    }

    if let Some(code) = params.code {
        *result.lock().await = Some(Ok(code));
        Html(
            r#"
            <html>
                <body>
                    <h1>Authorization Successful!</h1>
                    <p>You can close this window and return to your terminal.</p>
                </body>
            </html>
            "#
            .to_string(),
        )
    } else {
        *result.lock().await = Some(Err(anyhow::anyhow!("No authorization code received")));
        Html(
            r#"
            <html>
                <body>
                    <h1>Authorization Failed</h1>
                    <p>No authorization code received.</p>
                    <p>You can close this window.</p>
                </body>
            </html>
            "#
            .to_string(),
        )
    }
}
