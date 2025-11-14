# Production Environment Configuration Guide

## Overview

LibrAgent supports environment-based configuration in both development and production builds through `.env` files.

## Environment File Loading

### Development Mode (`pnpm tauri dev`)

1. Tries to load `.env.dev` first
2. Falls back to `.env` if `.env.dev` doesn't exist
3. Uses system environment variables and defaults if neither exists

### Production Mode (`pnpm tauri build`)

- Loads `.env` file from the **same directory as the executable**
- Falls back to system environment variables and defaults if `.env` doesn't exist

## Production Deployment

### Step 1: Build the Application

```bash
pnpm tauri build
```

### Step 2: Locate the Built Executable

**Linux:**

```
src-tauri/target/release/libr-agent
```

**Windows:**

```
src-tauri/target/release/libr-agent.exe
```

**macOS:**

```
src-tauri/target/release/bundle/macos/LibrAgent.app
```

### Step 3: Create Production .env File

Create a `.env` file in the **same directory as the executable**:

**For Linux/Windows:**

```bash
# src-tauri/target/release/.env
LIBRAGENT_MAX_FILE_SIZE=209715200
LIBRAGENT_DEFAULT_EXECUTION_TIMEOUT=60
LIBRAGENT_MAX_EXECUTION_TIMEOUT=600
RUST_LOG=info
```

**For macOS App Bundle:**

```bash
# Place .env in the Contents/MacOS/ directory
src-tauri/target/release/bundle/macos/LibrAgent.app/Contents/MacOS/.env
```

### Step 4: Verify Configuration Loading

Run the application and check the logs. You should see:

```
✅ Loaded .env file from: /path/to/.env
```

## Configuration Variables

All available environment variables:

```bash
# Maximum file size in bytes (default: 104857600 = 100MB)
LIBRAGENT_MAX_FILE_SIZE=104857600

# Default command execution timeout in seconds (default: 30)
LIBRAGENT_DEFAULT_EXECUTION_TIMEOUT=30

# Maximum command execution timeout in seconds (default: 300)
LIBRAGENT_MAX_EXECUTION_TIMEOUT=300

# Maximum process output size in bytes (default: 104857600 = 100MB)
LIBRAGENT_MAX_OUTPUT_SIZE=104857600

# Graceful shutdown timeout in seconds (default: 3)
LIBRAGENT_GRACEFUL_SHUTDOWN_TIMEOUT=3

# Excessive polling detection threshold (default: 5)
LIBRAGENT_POLL_THRESHOLD=5

# Message snippet length for search index (default: 200)
MESSAGE_INDEX_SNIPPET_LENGTH=200

# SQLite database path (optional - defaults to user data directory)
# LIBRAGENT_DB_PATH=/path/to/database.db

# Rust logging level (debug, info, warn, error)
RUST_LOG=info
```

## Testing Production Configuration

### Quick Test Script

Create a test `.env` file and verify it's loaded:

```bash
# 1. Create .env in the executable directory
cd src-tauri/target/release
cat > .env << EOF
LIBRAGENT_MAX_FILE_SIZE=209715200
RUST_LOG=debug
EOF

# 2. Run the application
./libr-agent

# 3. Check logs for: "✅ Loaded .env file from: ..."
```

### Verification Steps

1. **Create test .env with increased file size limit** (e.g., 200MB)
2. **Upload a large file** (e.g., 150MB) in the WorkspaceFilesPanel
3. **Success = .env is working**; Failure = using default 100MB limit

## Alternative: System Environment Variables

Instead of using `.env` files, you can set system environment variables:

**Linux/macOS:**

```bash
export LIBRAGENT_MAX_FILE_SIZE=209715200
export RUST_LOG=info
./libr-agent
```

**Windows (PowerShell):**

```powershell
$env:LIBRAGENT_MAX_FILE_SIZE="209715200"
$env:RUST_LOG="info"
.\libr-agent.exe
```

**Windows (cmd):**

```cmd
set LIBRAGENT_MAX_FILE_SIZE=209715200
set RUST_LOG=info
libr-agent.exe
```

## Docker Deployment

For Docker containers, use environment variables in docker-compose.yml:

```yaml
version: '3.8'
services:
  libragent:
    image: libragent:latest
    environment:
      - LIBRAGENT_MAX_FILE_SIZE=209715200
      - LIBRAGENT_DEFAULT_EXECUTION_TIMEOUT=60
      - RUST_LOG=info
```

## Security Notes

⚠️ **Important:**

- Add `.env` and `.env.dev` to `.gitignore`
- Never commit sensitive values (API keys, passwords) to version control
- Use different `.env` files for different environments
- Consider using secret management tools for production deployments

## Troubleshooting

### .env File Not Loading

**Problem:** No "✅ Loaded .env" message in logs

**Solutions:**

1. Ensure `.env` is in the same directory as the executable
2. Check file permissions (must be readable)
3. Verify file name is exactly `.env` (not `.env.txt`)
4. Check `RUST_LOG=debug` to see detailed loading logs

### Configuration Not Taking Effect

**Problem:** .env loads but values not applied

**Solutions:**

1. Check variable names exactly match (case-sensitive)
2. Verify numeric values are valid integers
3. Restart the application after changing `.env`
4. Check for typos in environment variable names

### Production Build Issues

**Problem:** Works in dev but not in production

**Solutions:**

1. Ensure `.env` file exists where the executable runs
2. For app bundles, place `.env` in the correct location
3. Try using system environment variables instead
4. Enable debug logging: `RUST_LOG=debug`

## Best Practices

1. **Use .env.example as template** - Copy and modify for each environment
2. **Document required variables** - Update .env.example when adding new config
3. **Separate dev/prod configs** - Use .env.dev for development, .env for production
4. **Version control .env.example only** - Never commit actual .env files
5. **Validate on startup** - Check critical variables are set correctly
6. **Use sensible defaults** - Application should work without .env for basic use cases

## Related Documentation

- [Configuration Module](../src-tauri/src/config.rs) - Runtime configuration loading
- [Environment Setup](./environment-setup.md) - Development environment guide
- [.env.example](../src-tauri/.env.example) - Template configuration file
