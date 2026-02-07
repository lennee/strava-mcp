# URGENT ACTIONS REQUIRED

## 1. Revoke Exposed Credentials IMMEDIATELY

The following credentials were found in your git history and must be revoked:

```
STRAVA_CLIENT_ID=199349
STRAVA_CLIENT_SECRET=0b7cbc562285018ebd2c794bc6d72dbc80bcc11e
```

**Action:** Go to https://www.strava.com/settings/api and:
1. Delete the application with Client ID `199349`
2. Create a new application
3. Note the new Client ID and Client Secret

## 2. Remove Credentials from Git History

Run these commands to remove the `.env` file from git history:

```bash
cd /Users/tristan.smith2/tristan-git/strava-mcp

# Remove .env from all commits
git filter-branch --force --index-filter \
  'git rm --cached --ignore-unmatch .env' \
  --prune-empty --tag-name-filter cat -- --all

# Force push to remote (if applicable)
# WARNING: This rewrites history. Coordinate with team members.
git push origin --force --all
git push origin --force --tags

# Clean up local repo
rm -rf .git/refs/original/
git reflog expire --expire=now --all
git gc --prune=now --aggressive
```

## 3. Set Up New Credentials

After creating a new Strava application:

```bash
# Copy the example file
cp .env.example .env

# Edit .env and add your NEW credentials
# STRAVA_CLIENT_ID=<your_new_client_id>
# STRAVA_CLIENT_SECRET=<your_new_client_secret>
```

**NEVER commit the .env file!** It's already in `.gitignore`.

## 4. Remove Old Token File (Optional)

If you have an old token file from the exposed credentials:

```bash
rm ~/.strava/token.json
```

You'll need to re-authorize on first use with the new credentials.

## 5. Verify Security Fixes

After setting up new credentials, test that security fixes are working:

```bash
# Build the project
cd /Users/tristan.smith2/tristan-git/strava-mcp
cargo build

# Check token file permissions (after first authorization)
ls -la ~/.strava/token.json
# Should show: -rw------- (600)

# Check directory permissions
ls -la ~/.strava
# Should show: drwx------ (700)
```

## Timeline

- **NOW:** Revoke old credentials
- **WITHIN 1 HOUR:** Remove from git history and force push
- **TODAY:** Set up new credentials and test

## Questions?

If you encounter issues:
1. Check that `.env` is in `.gitignore` ✅
2. Verify new credentials work on Strava's API settings page
3. Run `cargo build` to ensure no compilation errors
