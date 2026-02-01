#!/usr/bin/env node

import { config } from "dotenv";
import { fileURLToPath } from "url";
import { dirname, join } from "path";
import { writeFileSync, readFileSync } from "fs";
import { createServer } from "http";
import { exec } from "child_process";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

// Load .env from the package directory
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const envPath = join(__dirname, "..", ".env");
config({ path: envPath });

const STRAVA_API_BASE = "https://www.strava.com/api/v3";
const STRAVA_TOKEN_URL = "https://www.strava.com/oauth/token";
const STRAVA_AUTH_URL = "https://www.strava.com/oauth/authorize";

function openBrowser(url: string): void {
  const platform = process.platform;
  let command: string;

  if (platform === "darwin") {
    command = `open "${url}"`;
  } else if (platform === "win32") {
    command = `start "" "${url}"`;
  } else {
    command = `xdg-open "${url}"`;
  }

  exec(command, (error) => {
    if (error) {
      console.error("Failed to open browser:", error);
    }
  });
}

function saveTokensToEnv(data: {
  access_token: string;
  refresh_token: string;
  expires_at: number;
}): void {
  let envContent = readFileSync(envPath, "utf-8");

  const updates: Record<string, string> = {
    STRAVA_ACCESS_TOKEN: data.access_token,
    STRAVA_REFRESH_TOKEN: data.refresh_token,
    STRAVA_EXPIRES_AT: data.expires_at.toString(),
  };

  for (const [key, value] of Object.entries(updates)) {
    const regex = new RegExp(`^${key}=.*$`, "m");
    if (regex.test(envContent)) {
      envContent = envContent.replace(regex, `${key}=${value}`);
    } else {
      envContent += `\n${key}=${value}`;
    }
  }

  writeFileSync(envPath, envContent);

  process.env.STRAVA_ACCESS_TOKEN = data.access_token;
  process.env.STRAVA_REFRESH_TOKEN = data.refresh_token;
  process.env.STRAVA_EXPIRES_AT = data.expires_at.toString();
}

// Token state
let cachedAccessToken: string | null = process.env.STRAVA_ACCESS_TOKEN || null;
let tokenExpiresAt: number = parseInt(process.env.STRAVA_EXPIRES_AT || "0", 10);

async function refreshAccessToken(): Promise<string> {
  const clientId = process.env.STRAVA_CLIENT_ID;
  const clientSecret = process.env.STRAVA_CLIENT_SECRET;
  const refreshToken = process.env.STRAVA_REFRESH_TOKEN;

  if (!clientId || !clientSecret || !refreshToken) {
    throw new Error(
      "Missing STRAVA_CLIENT_ID, STRAVA_CLIENT_SECRET, or STRAVA_REFRESH_TOKEN in .env"
    );
  }

  const response = await fetch(STRAVA_TOKEN_URL, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      client_id: clientId,
      client_secret: clientSecret,
      grant_type: "refresh_token",
      refresh_token: refreshToken,
    }),
  });

  if (!response.ok) {
    const error = await response.text();
    throw new Error(`Token refresh failed: ${response.status} - ${error}`);
  }

  const data = await response.json();

  // Update cached values
  cachedAccessToken = data.access_token;
  tokenExpiresAt = data.expires_at;

  // Persist new tokens to .env
  try {
    saveTokensToEnv(data);
  } catch (err) {
    // Log but don't fail - token still works for this session
    console.error("Warning: Could not persist tokens to .env:", err);
  }

  return data.access_token;
}

async function getValidAccessToken(): Promise<string> {
  const now = Math.floor(Date.now() / 1000);

  // Refresh if token expires in less than 5 minutes
  if (!cachedAccessToken || tokenExpiresAt < now + 300) {
    return await refreshAccessToken();
  }

  return cachedAccessToken;
}

interface StravaActivity {
  id: number;
  name: string;
  type: string;
  sport_type: string;
  start_date: string;
  start_date_local: string;
  distance: number; // meters
  moving_time: number; // seconds
  elapsed_time: number; // seconds
  total_elevation_gain: number; // meters
  average_speed: number; // m/s
  max_speed: number; // m/s
  average_heartrate?: number;
  max_heartrate?: number;
  suffer_score?: number;
}

function formatDuration(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m ${secs}s`;
  }
  return `${minutes}m ${secs}s`;
}

function formatPace(metersPerSecond: number): string {
  if (metersPerSecond === 0) return "N/A";
  const minutesPerKm = 1000 / metersPerSecond / 60;
  const mins = Math.floor(minutesPerKm);
  const secs = Math.round((minutesPerKm - mins) * 60);
  return `${mins}:${secs.toString().padStart(2, "0")} /km`;
}

function formatDistance(meters: number): string {
  const km = meters / 1000;
  return `${km.toFixed(2)} km`;
}

async function fetchActivities(
  accessToken: string,
  after?: number,
  before?: number
): Promise<StravaActivity[]> {
  const params = new URLSearchParams();
  if (after) params.set("after", after.toString());
  if (before) params.set("before", before.toString());
  params.set("per_page", "100");

  const response = await fetch(
    `${STRAVA_API_BASE}/athlete/activities?${params}`,
    {
      headers: {
        Authorization: `Bearer ${accessToken}`,
      },
    }
  );

  if (!response.ok) {
    const error = await response.text();
    throw new Error(`Strava API error: ${response.status} - ${error}`);
  }

  return response.json();
}

const server = new McpServer({
  name: "strava",
  version: "1.0.0",
});

server.tool(
  "get_runs_for_date",
  "Get running activities for a specific date. Returns distance, duration, pace, and other stats.",
  {
    date: z
      .string()
      .describe("Date in YYYY-MM-DD format (e.g., 2026-01-31)"),
    access_token: z
      .string()
      .optional()
      .describe(
        "Strava API access token. If not provided, uses STRAVA_ACCESS_TOKEN env var."
      ),
  },
  async ({ date, access_token }) => {
    let token: string;
    try {
      token = access_token || (await getValidAccessToken());
    } catch (err) {
      return {
        content: [
          {
            type: "text" as const,
            text: `Error: ${err instanceof Error ? err.message : "Failed to get access token"}`,
          },
        ],
      };
    }

    const targetDate = new Date(date);
    if (isNaN(targetDate.getTime())) {
      return {
        content: [
          {
            type: "text" as const,
            text: `Error: Invalid date format "${date}". Use YYYY-MM-DD format.`,
          },
        ],
      };
    }

    // Set time boundaries for the target date (local timezone)
    const startOfDay = new Date(targetDate);
    startOfDay.setHours(0, 0, 0, 0);
    const endOfDay = new Date(targetDate);
    endOfDay.setHours(23, 59, 59, 999);

    const after = Math.floor(startOfDay.getTime() / 1000);
    const before = Math.floor(endOfDay.getTime() / 1000);

    const activities = await fetchActivities(token, after, before);

    // Filter for runs only
    const runs = activities.filter(
      (a) =>
        a.type === "Run" ||
        a.sport_type === "Run" ||
        a.sport_type === "TrailRun"
    );

    if (runs.length === 0) {
      return {
        content: [
          {
            type: "text" as const,
            text: `No runs found for ${date}.`,
          },
        ],
      };
    }

    let totalDistance = 0;
    let totalTime = 0;

    const runDetails = runs.map((run) => {
      totalDistance += run.distance;
      totalTime += run.moving_time;

      let details = `**${run.name}**
- Distance: ${formatDistance(run.distance)}
- Duration: ${formatDuration(run.moving_time)}
- Pace: ${formatPace(run.average_speed)}
- Elevation gain: ${run.total_elevation_gain.toFixed(0)}m`;

      if (run.average_heartrate) {
        details += `\n- Avg HR: ${run.average_heartrate.toFixed(0)} bpm`;
      }
      if (run.max_heartrate) {
        details += `\n- Max HR: ${run.max_heartrate.toFixed(0)} bpm`;
      }

      return details;
    });

    const summary =
      runs.length > 1
        ? `\n\n**Daily Total:**
- Runs: ${runs.length}
- Total distance: ${formatDistance(totalDistance)}
- Total time: ${formatDuration(totalTime)}`
        : "";

    return {
      content: [
        {
          type: "text" as const,
          text: `## Runs for ${date}\n\n${runDetails.join("\n\n")}${summary}`,
        },
      ],
    };
  }
);

server.tool(
  "get_recent_runs",
  "Get recent running activities. Returns the most recent runs with their stats.",
  {
    limit: z
      .number()
      .optional()
      .default(5)
      .describe("Maximum number of runs to return (default: 5)"),
    access_token: z
      .string()
      .optional()
      .describe(
        "Strava API access token. If not provided, uses STRAVA_ACCESS_TOKEN env var."
      ),
  },
  async ({ limit, access_token }) => {
    let token: string;
    try {
      token = access_token || (await getValidAccessToken());
    } catch (err) {
      return {
        content: [
          {
            type: "text" as const,
            text: `Error: ${err instanceof Error ? err.message : "Failed to get access token"}`,
          },
        ],
      };
    }

    const activities = await fetchActivities(token);

    // Filter for runs only
    const runs = activities
      .filter(
        (a) =>
          a.type === "Run" ||
          a.sport_type === "Run" ||
          a.sport_type === "TrailRun"
      )
      .slice(0, limit);

    if (runs.length === 0) {
      return {
        content: [
          {
            type: "text" as const,
            text: "No recent runs found.",
          },
        ],
      };
    }

    const runDetails = runs.map((run) => {
      const runDate = new Date(run.start_date_local).toLocaleDateString(
        "en-US",
        {
          weekday: "short",
          year: "numeric",
          month: "short",
          day: "numeric",
        }
      );

      let details = `**${run.name}** (${runDate})
- Distance: ${formatDistance(run.distance)}
- Duration: ${formatDuration(run.moving_time)}
- Pace: ${formatPace(run.average_speed)}`;

      if (run.average_heartrate) {
        details += `\n- Avg HR: ${run.average_heartrate.toFixed(0)} bpm`;
      }

      return details;
    });

    return {
      content: [
        {
          type: "text" as const,
          text: `## Recent Runs\n\n${runDetails.join("\n\n")}`,
        },
      ],
    };
  }
);

server.tool(
  "get_weekly_summary",
  "Get a summary of running activities for the current or specified week.",
  {
    week_start: z
      .string()
      .optional()
      .describe(
        "Start of week in YYYY-MM-DD format (defaults to current week's Monday)"
      ),
    access_token: z
      .string()
      .optional()
      .describe(
        "Strava API access token. If not provided, uses STRAVA_ACCESS_TOKEN env var."
      ),
  },
  async ({ week_start, access_token }) => {
    let token: string;
    try {
      token = access_token || (await getValidAccessToken());
    } catch (err) {
      return {
        content: [
          {
            type: "text" as const,
            text: `Error: ${err instanceof Error ? err.message : "Failed to get access token"}`,
          },
        ],
      };
    }

    let startDate: Date;
    if (week_start) {
      startDate = new Date(week_start);
    } else {
      // Default to Monday of current week
      startDate = new Date();
      const day = startDate.getDay();
      const diff = startDate.getDate() - day + (day === 0 ? -6 : 1);
      startDate.setDate(diff);
    }
    startDate.setHours(0, 0, 0, 0);

    const endDate = new Date(startDate);
    endDate.setDate(endDate.getDate() + 6);
    endDate.setHours(23, 59, 59, 999);

    const after = Math.floor(startDate.getTime() / 1000);
    const before = Math.floor(endDate.getTime() / 1000);

    const activities = await fetchActivities(token, after, before);

    const runs = activities.filter(
      (a) =>
        a.type === "Run" ||
        a.sport_type === "Run" ||
        a.sport_type === "TrailRun"
    );

    if (runs.length === 0) {
      const weekStr = startDate.toLocaleDateString("en-US", {
        month: "short",
        day: "numeric",
      });
      const weekEndStr = endDate.toLocaleDateString("en-US", {
        month: "short",
        day: "numeric",
      });
      return {
        content: [
          {
            type: "text" as const,
            text: `No runs found for week of ${weekStr} - ${weekEndStr}.`,
          },
        ],
      };
    }

    let totalDistance = 0;
    let totalTime = 0;
    let totalElevation = 0;

    runs.forEach((run) => {
      totalDistance += run.distance;
      totalTime += run.moving_time;
      totalElevation += run.total_elevation_gain;
    });

    const avgPace = totalDistance > 0 ? totalDistance / totalTime : 0;

    const weekStr = startDate.toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
    });
    const weekEndStr = endDate.toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
    });

    return {
      content: [
        {
          type: "text" as const,
          text: `## Weekly Running Summary (${weekStr} - ${weekEndStr})

- **Total runs:** ${runs.length}
- **Total distance:** ${formatDistance(totalDistance)}
- **Total time:** ${formatDuration(totalTime)}
- **Average pace:** ${formatPace(avgPace)}
- **Total elevation:** ${totalElevation.toFixed(0)}m`,
        },
      ],
    };
  }
);

server.tool(
  "authorize",
  "Authorize the Strava MCP with your Strava account. Opens a browser for OAuth login and automatically captures the tokens.",
  {
    port: z
      .number()
      .optional()
      .default(8089)
      .describe("Local port for OAuth callback (default: 8089)"),
    scope: z
      .string()
      .optional()
      .default("activity:read_all")
      .describe("OAuth scopes to request (default: activity:read_all)"),
  },
  async ({ port, scope }) => {
    const clientId = process.env.STRAVA_CLIENT_ID;
    const clientSecret = process.env.STRAVA_CLIENT_SECRET;

    if (!clientId || !clientSecret) {
      return {
        content: [
          {
            type: "text" as const,
            text: "Error: Missing STRAVA_CLIENT_ID or STRAVA_CLIENT_SECRET in .env file",
          },
        ],
      };
    }

    return new Promise((resolve) => {
      const redirectUri = `http://localhost:${port}/callback`;

      const server = createServer(async (req, res) => {
        const url = new URL(req.url || "", `http://localhost:${port}`);

        if (url.pathname === "/callback") {
          const code = url.searchParams.get("code");
          const error = url.searchParams.get("error");

          if (error) {
            res.writeHead(200, { "Content-Type": "text/html" });
            res.end(
              "<html><body><h1>Authorization Failed</h1><p>You can close this window.</p></body></html>"
            );
            server.close();
            resolve({
              content: [
                {
                  type: "text" as const,
                  text: `Authorization failed: ${error}`,
                },
              ],
            });
            return;
          }

          if (!code) {
            res.writeHead(200, { "Content-Type": "text/html" });
            res.end(
              "<html><body><h1>No Code Received</h1><p>You can close this window.</p></body></html>"
            );
            server.close();
            resolve({
              content: [
                {
                  type: "text" as const,
                  text: "No authorization code received",
                },
              ],
            });
            return;
          }

          // Exchange code for tokens
          try {
            const tokenResponse = await fetch(STRAVA_TOKEN_URL, {
              method: "POST",
              headers: { "Content-Type": "application/x-www-form-urlencoded" },
              body: new URLSearchParams({
                client_id: clientId,
                client_secret: clientSecret,
                code: code,
                grant_type: "authorization_code",
              }),
            });

            if (!tokenResponse.ok) {
              const errorText = await tokenResponse.text();
              res.writeHead(200, { "Content-Type": "text/html" });
              res.end(
                "<html><body><h1>Token Exchange Failed</h1><p>You can close this window.</p></body></html>"
              );
              server.close();
              resolve({
                content: [
                  {
                    type: "text" as const,
                    text: `Token exchange failed: ${tokenResponse.status} - ${errorText}`,
                  },
                ],
              });
              return;
            }

            const data = await tokenResponse.json();

            // Update cached values
            cachedAccessToken = data.access_token;
            tokenExpiresAt = data.expires_at;

            // Save tokens
            saveTokensToEnv(data);

            res.writeHead(200, { "Content-Type": "text/html" });
            res.end(
              `<html><body><h1>Authorization Successful!</h1><p>Welcome, ${data.athlete?.firstname}! You can close this window.</p></body></html>`
            );
            server.close();

            resolve({
              content: [
                {
                  type: "text" as const,
                  text: `## Authorization Successful!

Tokens have been saved to .env file.

- **Athlete:** ${data.athlete?.firstname} ${data.athlete?.lastname}
- **Scopes:** ${scope}
- **Expires:** ${new Date(data.expires_at * 1000).toLocaleString()}

You can now use the Strava tools to fetch your running data.`,
                },
              ],
            });
          } catch (err) {
            res.writeHead(200, { "Content-Type": "text/html" });
            res.end(
              "<html><body><h1>Error</h1><p>You can close this window.</p></body></html>"
            );
            server.close();
            resolve({
              content: [
                {
                  type: "text" as const,
                  text: `Error during token exchange: ${err instanceof Error ? err.message : "Unknown error"}`,
                },
              ],
            });
          }
        } else {
          res.writeHead(404);
          res.end("Not found");
        }
      });

      server.listen(port, () => {
        const authParams = new URLSearchParams({
          client_id: clientId,
          redirect_uri: redirectUri,
          response_type: "code",
          scope: scope,
          approval_prompt: "force",
        });

        const authUrl = `${STRAVA_AUTH_URL}?${authParams.toString()}`;

        // Open browser
        openBrowser(authUrl);
      });

      // Timeout after 2 minutes
      setTimeout(() => {
        server.close();
        resolve({
          content: [
            {
              type: "text" as const,
              text: "Authorization timed out. Please try again.",
            },
          ],
        });
      }, 120000);
    });
  }
);

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch(console.error);
