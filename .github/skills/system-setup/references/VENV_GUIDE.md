# Virtual Environment Guide

## Why Virtual Environments?

Virtual environments isolate Python dependencies per project, preventing:
- Version conflicts between projects
- Global package pollution
- Permission issues with system Python
- "It works on my machine" problems

## Creating Virtual Environments

### Python venv (Built-in)

**All Platforms:**
```bash
# Create venv
python -m venv myenv

# Activate
source myenv/bin/activate  # Linux/macOS
.\myenv\Scripts\activate   # Windows PowerShell
myenv\Scripts\activate.bat # Windows CMD

# Deactivate
deactivate
```

### uv (Recommended for Speed)

**All Platforms:**
```bash
# Create venv with uv (much faster)
uv venv

# Activate
source .venv/bin/activate  # Linux/macOS
.\.venv\Scripts\activate   # Windows

# Install packages
uv pip install package-name

# Generate requirements
uv pip freeze > requirements.txt
```

## Virtual Environment Locations

**Standard locations:**
- Project root: `./venv` or `./.venv`
- Centralized: `~/.virtualenvs/project-name/`

**LibrAgent MCP servers:**
- Each MCP server may have its own venv
- Check `mcp-server-directory/.venv/`

## Managing Dependencies

### Installing Packages

```bash
# Activate venv first!
source .venv/bin/activate

# Using pip
pip install package-name

# Using uv (faster)
uv pip install package-name

# From requirements.txt
pip install -r requirements.txt
uv pip install -r requirements.txt
```

### Exporting Dependencies

```bash
# Generate requirements.txt
pip freeze > requirements.txt
uv pip freeze > requirements.txt

# With version hashes (more secure)
pip freeze --all > requirements.txt
```

## IDE Integration

### VS Code

Add to `.vscode/settings.json`:
```json
{
  "python.defaultInterpreterPath": "${workspaceFolder}/.venv/bin/python",
  "python.terminal.activateEnvironment": true
}
```

### PyCharm

1. File → Settings → Project → Python Interpreter
2. Click gear icon → Add
3. Select "Virtualenv Environment" → "Existing environment"
4. Choose `.venv/bin/python`

## Common Issues

### Activation Not Working

**Symptoms:** `(venv)` prefix doesn't appear

**Solutions:**
- Ensure correct activation script path
- Check script execution policy (Windows): `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned`
- Try absolute path: `C:\path\to\venv\Scripts\activate`

### Wrong Python Version

**Symptoms:** Venv uses different Python than expected

**Solutions:**
```bash
# Specify Python version explicitly
python3.11 -m venv myenv
/usr/bin/python3.11 -m venv myenv
C:\Python311\python.exe -m venv myenv
```

### Packages Not Found After Activation

**Symptoms:** `ModuleNotFoundError` despite installing package

**Solutions:**
- Verify venv is activated: `which python` or `Get-Command python`
- Check pip installation: `pip list`
- Reinstall package in activated venv

### Venv Too Large

**Symptoms:** Venv directory consumes excessive disk space

**Solutions:**
- Don't commit venv to git (add to `.gitignore`)
- Use `--without-pip` and install pip later if needed
- Clean up unused packages: `pip uninstall package-name`

## Best Practices

### Project Structure

```
project/
├── .venv/           # Virtual environment (gitignored)
├── src/             # Source code
├── requirements.txt # Dependencies
└── README.md
```

### Gitignore

Add to `.gitignore`:
```
# Virtual environments
venv/
.venv/
env/
ENV/
```

### Requirements.txt Management

**Split requirements:**
- `requirements.txt` - Production dependencies
- `requirements-dev.txt` - Development tools (pytest, black, etc.)

Install both:
```bash
pip install -r requirements.txt
pip install -r requirements-dev.txt
```

### Reproducible Environments

```bash
# Pin versions
pip freeze > requirements.txt

# Or use pip-tools for deterministic builds
pip install pip-tools
pip-compile requirements.in
pip-sync requirements.txt
```

## MCP Server Contexts

LibrAgent MCP servers may use venvs:

**Check MCP server venv:**
```bash
# Look for .venv or venv directory
ls -la /path/to/mcp-server/

# Check which Python is used
cat /path/to/mcp-server/start.sh
```

**Activate MCP server venv manually:**
```bash
cd /path/to/mcp-server
source .venv/bin/activate
python server.py
```

## uv Virtual Environments

uv provides faster venv operations:

```bash
# Create venv
uv venv

# Install packages (parallel, cached)
uv pip install -r requirements.txt

# Sync exact dependencies
uv pip sync requirements.txt

# Compile requirements with hashes
uv pip compile requirements.in -o requirements.txt
```

**Benefits:**
- 10-100x faster than pip
- Parallel downloads
- Disk cache for offline installs
- Drop-in pip replacement
