# Strava MCP Server

A Model Context Protocol (MCP) server for Strava API integration, written in Rust. This server enables AI models like Claude to fetch and analyze running activity data from the Strava API.

## Features

- **OAuth 2.0 Authentication**: Secure authorization flow with automatic token refresh
- **Four MCP Tools**:
  - `get_runs_for_date`: Get running activities for a specific date
  - `get_recent_runs`: Fetch most recent running activities
  - `get_weekly_summary`: Generate weekly running statistics
  - `authorize`: Authorize the MCP with your Strava account
- **Automatic Token Management**: Token caching and automatic refresh before expiration
- **Cross-Platform**: Works on macOS, Windows, and Linux

## Requirements

- Rust 1.75 or later
- A Strava account
- Strava API application (Client ID and Client Secret)

## Installation

### 1. Clone the Repository

```bash
git clone https://github.com/yourusername/strava-mcp.git
cd strava-mcp
```

### 2. Create a Strava API Application

1. Go to https://www.strava.com/settings/api
2. Create a new application
3. Note your **Client ID** and **Client Secret**
4. Set the Authorization Callback Domain to `localhost`

### 3. Configure Environment Variables

Create a `.env` file in the project root:

```bash
STRAVA_CLIENT_ID=your_client_id_here
STRAVA_CLIENT_SECRET=your_client_secret_here
```

The `authorize` tool will automatically add the access tokens to this file.

### 4. Build the Project

```bash
cargo build --release
```

## Usage

### Running the Server

```bash
# Development mode
cargo run

# Or run the compiled binary
./target/release/strava-mcp
```

### Using with Claude Desktop

Add to your Claude Desktop MCP configuration:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "strava": {
      "command": "/path/to/strava-mcp/target/release/strava-mcp",
      "env": {
        "STRAVA_CLIENT_ID": "your_client_id",
        "STRAVA_CLIENT_SECRET": "your_client_secret"
      }
    }
  }
}
```

### First Time Setup

1. Start the MCP server
2. Call the `authorize` tool through your MCP client
3. Complete the OAuth flow in your browser
4. The access tokens will be saved to `.env`

## Available Tools

### `authorize`

Initiates OAuth flow to authorize the MCP with your Strava account.

**Parameters:**
- `port` (optional): Port for OAuth callback server (default: 8089)
- `scope` (optional): OAuth scope (default: "activity:read_all")

**Example:**
```
authorize with port 8089
```

### `get_runs_for_date`

Get all running activities for a specific date.

**Parameters:**
- `date` (required): Date in YYYY-MM-DD format
- `access_token` (optional): Strava access token (uses cached token if not provided)

**Example:**
```
get runs for 2024-01-15
```

### `get_recent_runs`

Get the most recent running activities.

**Parameters:**
- `limit` (optional): Number of runs to retrieve (default: 5)
- `access_token` (optional): Strava access token

**Example:**
```
get my 10 most recent runs
```

### `get_weekly_summary`

Get aggregated running statistics for a week.

**Parameters:**
- `week_start` (optional): Start of week in YYYY-MM-DD format (defaults to current Monday)
- `access_token` (optional): Strava access token

**Example:**
```
get weekly summary for week starting 2024-01-15
```

## Development

### Running Tests

```bash
cargo test
```

### Code Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

### Development with Auto-Reload

Install cargo-watch:

```bash
cargo install cargo-watch
```

Run with auto-reload:

```bash
cargo watch -x run
```

## Architecture

- **`src/main.rs`**: MCP server setup and tool implementations
- **`src/models.rs`**: Data structures (StravaActivity, TokenCache, etc.)
- **`src/oauth.rs`**: OAuth flow and token management
- **`src/strava_api.rs`**: HTTP client for Strava API
- **`src/utils.rs`**: Formatting utilities (duration, pace, distance)

## Token Management

The server implements intelligent token management:

- Tokens are cached in memory for performance
- Automatic refresh when tokens are expiring (within 5 minutes)
- Tokens are persisted to `.env` file for reuse across sessions
- Thread-safe token access using `Arc<Mutex<...>>`

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Troubleshooting

### "STRAVA_CLIENT_ID environment variable not set"

Make sure you have created a `.env` file with your Strava API credentials.

### "No access token available"

Run the `authorize` tool to complete the OAuth flow and obtain an access token.

### "Failed to refresh access token"

Your refresh token may have expired. Run the `authorize` tool again to obtain new tokens.

### Port Already in Use

If port 8089 is already in use, you can specify a different port with the `authorize` tool:

```
authorize with port 8090
```
