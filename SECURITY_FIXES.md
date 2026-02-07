# Security Fixes Applied

This document summarizes the security vulnerabilities that were fixed in this codebase.

## Date: 2025-02-07

## Critical Issues Fixed

### 1. Exposed Credentials in Version Control ✅
**Severity:** CRITICAL
**Location:** `.env` file
**Issue:** Production credentials were committed to git repository
**Fix:**
- Cleared credentials from `.env` file
- Created `.env.example` template
- **ACTION REQUIRED:** You must:
  1. Revoke exposed credentials at https://www.strava.com/settings/api
  2. Generate new credentials
  3. Remove from git history using: `git filter-branch --force --index-filter 'git rm --cached --ignore-unmatch .env' --prune-empty --tag-name-filter cat -- --all`
  4. Add new credentials to `.env` locally (never commit)

### 2. World-Readable Token Files ✅
**Severity:** CRITICAL
**Location:** `strava-api/src/oauth/persistence.rs`
**Issue:** OAuth tokens stored with default permissions (readable by all users)
**Fix:**
- Set file permissions to `0o600` (owner read/write only)
- Set directory permissions to `0o700` (owner access only)
- Applied on Unix systems using `std::os::unix::fs::PermissionsExt`

### 3. Token Override Vulnerability ✅
**Severity:** CRITICAL
**Location:** `strava-mcp/src/main.rs`
**Issue:** All tools accepted optional `access_token` parameter allowing arbitrary token injection
**Fix:**
- Removed `access_token` parameter from all tool structs
- Changed all methods to use only authenticated client
- Prevents cross-user access and unauthorized token usage

## High Severity Issues Fixed

### 4. Missing OAuth CSRF Protection ✅
**Severity:** HIGH
**Location:** `strava-api/src/oauth/manager.rs`
**Issue:** No state parameter in OAuth flow (CSRF vulnerability)
**Fix:**
- Generate random 32-character state parameter
- Include in authorization URL
- Validate state matches in callback
- Reject requests with missing or mismatched state

### 5. XSS in OAuth Callback Response ✅
**Severity:** HIGH
**Location:** `strava-api/src/oauth/manager.rs`
**Issue:** Error parameter displayed in HTML without escaping
**Fix:**
- HTML-escape error messages before rendering
- Escape `&`, `<`, `>`, `"`, `'` characters
- Prevents JavaScript injection via error parameter

### 6. Unbounded Input Parameters ✅
**Severity:** HIGH
**Location:** `strava-mcp/src/main.rs`
**Issue:** No validation on `limit`, `port`, `scope`, and `date` parameters
**Fix:**
- **limit:** Bounded to 1-100, default 5
- **port:** Bounded to 1024-65535, prevents privileged ports
- **scope:** Whitelist of valid OAuth scopes
- **date:** Length validation, range validation (2009-present)

### 7. Race Condition in Token Refresh ✅
**Severity:** HIGH
**Location:** `strava-api/src/oauth/manager.rs`
**Issue:** Multiple concurrent threads could trigger simultaneous token refreshes
**Fix:**
- Added `refresh_in_progress` flag with Mutex
- Threads wait and retry if refresh is in progress
- Only one thread performs refresh at a time
- Others use refreshed token when available

### 8. TOCTOU Race in Directory Creation ✅
**Severity:** HIGH
**Location:** `strava-api/src/oauth/persistence.rs`
**Issue:** Directory existence check before creation (symlink attack vulnerability)
**Fix:**
- Removed existence check before `create_dir_all`
- `create_dir_all` is atomic and handles existing directories
- Set restrictive permissions immediately after creation

### 9. Path Validation in OAuth Callback ✅
**Severity:** HIGH
**Location:** `strava-api/src/oauth/manager.rs`
**Issue:** Callback handler didn't validate request path
**Fix:**
- Added path validation to ensure requests are to `/callback`
- Ignore non-callback requests
- Added URL decoding for query parameters

## Dependencies Added

Security-related dependencies added:
- `rand = "0.8"` - Cryptographically secure random number generation for CSRF tokens
- `urlencoding = "2.1"` - Proper URL parameter decoding

## Remaining Considerations

### Token Encryption at Rest (Not Implemented)
**Severity:** HIGH
**Status:** Pending
**Issue:** Tokens still stored as plain text JSON
**Recommendation:** Consider implementing AES-256 encryption for token storage using `aes-gcm` or OS keyring integration via `keyring-rs`

### Advanced Protections (Optional)
- **PKCE (Proof Key for Code Exchange):** Not implemented but recommended for additional OAuth security
- **Rate Limiting:** No rate limiting on authorization attempts
- **Audit Logging:** No logging of credential usage

## Testing Recommendations

1. **File Permissions Test:**
   ```bash
   ls -la ~/.strava/token.json
   # Should show: -rw------- (600)
   ```

2. **CSRF Test:** Attempt to use authorization callback with wrong state parameter

3. **Input Validation Test:** Try parameters outside valid ranges:
   - limit: 0, 1000
   - port: 80, 100000
   - date: "1990-01-01", "2030-01-01"

4. **Race Condition Test:** Concurrent calls to endpoints requiring token refresh

## Security Best Practices Going Forward

1. **Never commit credentials** - Always use `.env` for local development
2. **Rotate credentials regularly** - Change OAuth secrets periodically
3. **Monitor token file** - Ensure `~/.strava/token.json` has correct permissions
4. **Review dependencies** - Run `cargo audit` regularly to check for CVEs
5. **Update dependencies** - Keep all crates up to date with security patches

## Summary

**Fixed:**
- 3 Critical vulnerabilities
- 6 High severity vulnerabilities

**Total:** 9 security issues resolved

All code compiles successfully with only minor unused import warnings.
