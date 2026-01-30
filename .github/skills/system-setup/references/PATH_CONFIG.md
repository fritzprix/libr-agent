# PATH Configuration Guide

## Understanding PATH

The PATH environment variable tells the operating system where to find executable files. When you type a command like `python` or `node`, the system searches directories listed in PATH.

## Viewing Current PATH

**Windows PowerShell:**
```powershell
$env:PATH -split ';'
```

**Linux/macOS:**
```bash
echo $PATH | tr ':' '\n'
```

## Adding to PATH

### Windows

**Method 1: System Settings (Permanent)**
1. Search "Environment Variables" in Start menu
2. Click "Environment Variables"
3. Under "User variables" or "System variables", select "Path"
4. Click "Edit" → "New"
5. Add directory path (e.g., `C:\Python311\Scripts`)
6. Click OK and restart terminal

**Method 2: PowerShell (Current Session)**
```powershell
$env:PATH += ";C:\path\to\directory"
```

**Method 3: PowerShell Profile (Permanent)**
```powershell
# Edit profile
notepad $PROFILE

# Add this line:
$env:PATH += ";C:\path\to\directory"
```

### Linux/macOS

**Method 1: Shell RC File (Permanent)**

For Bash (`~/.bashrc` or `~/.bash_profile`):
```bash
export PATH="$HOME/.local/bin:$PATH"
```

For Zsh (`~/.zshrc`):
```bash
export PATH="$HOME/.local/bin:$PATH"
```

Apply changes:
```bash
source ~/.bashrc  # or ~/.zshrc
```

**Method 2: Current Session Only**
```bash
export PATH="$HOME/.local/bin:$PATH"
```

## Common PATH Locations

### Python

**Windows:**
- `C:\Python311\`
- `C:\Python311\Scripts\`
- `%APPDATA%\Python\Python311\Scripts\`

**Linux:**
- `/usr/bin/`
- `/usr/local/bin/`
- `~/.local/bin/`

**macOS:**
- `/usr/local/bin/`
- `/opt/homebrew/bin/` (Apple Silicon)
- `~/Library/Python/3.11/bin/`

### Node.js

**Windows:**
- `C:\Program Files\nodejs\`
- `%APPDATA%\npm\`

**Linux:**
- `/usr/bin/`
- `/usr/local/bin/`
- `~/.npm-global/bin/`

**macOS:**
- `/usr/local/bin/`
- `/opt/homebrew/bin/`

### uv

**Windows:**
- `%USERPROFILE%\.cargo\bin\`

**Linux/macOS:**
- `~/.cargo/bin/`

## Troubleshooting

### PATH Not Updating

**Windows:**
- Close all terminal windows and reopen
- Log out and log back in
- Restart computer (system-wide changes)

**Linux/macOS:**
- Run `source ~/.bashrc` or `source ~/.zshrc`
- Close terminal and open new one
- Check shell type: `echo $SHELL`

### Duplicate Entries

**Windows:**
```powershell
# Remove duplicates
$paths = $env:PATH -split ';' | Select-Object -Unique
$env:PATH = $paths -join ';'
```

**Linux/macOS:**
```bash
# View duplicates
echo $PATH | tr ':' '\n' | sort | uniq -d
```

### Permission Denied

**Linux/macOS:**
- Ensure directories have execute permissions: `chmod +x /path/to/executable`
- Check file ownership: `ls -la /path/to/executable`

### Wrong Version Executed

Multiple versions installed? Check which one is found first:

```bash
# Windows
Get-Command python -All

# Linux/macOS
which -a python3
```

Order matters: The first match in PATH is executed.

## Security Considerations

- **Avoid adding current directory (`.`) to PATH** - Security risk
- **Use absolute paths** - Avoid relative paths in PATH
- **Limit write permissions** - Ensure PATH directories aren't world-writable
- **Order matters** - Place trusted directories first

## PATH Priority

Directories are searched in order:
1. **Windows:** Left to right (separated by `;`)
2. **Linux/macOS:** Left to right (separated by `:`)

To prioritize a specific installation, place it earlier in PATH.
