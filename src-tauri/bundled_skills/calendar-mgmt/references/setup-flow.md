# Calendar Setup Flow

Do **not** ask for `client_secret` in chat. Follow the two-step pattern used by `email-integration`.

## PowerShell (LibrAgent)

### First-time connect

1. Hidden prompt (`requireUserInput=true`, `inputType=password`):

```powershell
$env:LIBRAGENT_GOOGLE_CLIENT_SECRET = Read-Host "Google OAuth client secret"
```

2. Visible setup + OAuth:

```powershell
python "<skill-base-dir>/scripts/setup.py" `
  --client-id "YOUR_CLIENT_ID.apps.googleusercontent.com" `
  --client-secret-env LIBRAGENT_GOOGLE_CLIENT_SECRET `
  --oauth
```

3. Cleanup:

```powershell
Remove-Item Env:LIBRAGENT_GOOGLE_CLIENT_SECRET -ErrorAction SilentlyContinue
```

### Re-auth (tokens expired / revoked)

If `validate_config.py` returns `action: oauth` and client credentials are already saved:

```powershell
$env:LIBRAGENT_GOOGLE_CLIENT_SECRET = Read-Host "Google OAuth client secret"
python "<skill-base-dir>/scripts/setup.py" --oauth --client-secret-env LIBRAGENT_GOOGLE_CLIENT_SECRET
Remove-Item Env:LIBRAGENT_GOOGLE_CLIENT_SECRET -ErrorAction SilentlyContinue
```

### Full reset

```powershell
$env:LIBRAGENT_GOOGLE_CLIENT_SECRET = Read-Host "Google OAuth client secret"
python "<skill-base-dir>/scripts/setup.py" `
  --reset `
  --client-id "YOUR_CLIENT_ID.apps.googleusercontent.com" `
  --client-secret-env LIBRAGENT_GOOGLE_CLIENT_SECRET `
  --oauth
Remove-Item Env:LIBRAGENT_GOOGLE_CLIENT_SECRET -ErrorAction SilentlyContinue
```

Replace `<skill-base-dir>` with this skill's absolute Base Directory.

On bash, use the same env-var pattern or export before running `setup.py`.
