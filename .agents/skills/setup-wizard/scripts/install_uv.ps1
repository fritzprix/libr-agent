# uv Installation Script for Windows
# Non-interactive. User scope only: uv installs under the user profile, so
# this script never needs an elevated shell.

param(
    [switch]$Force = $false
)

$ErrorActionPreference = "Stop"
# Absolute paths so the hints work from any current folder.
$scriptDir = $PSScriptRoot

Write-Host "=== uv Installation Script ===" -ForegroundColor Cyan
Write-Host ""

# A working uv is enough unless -Force is given.
$uvCmd = Get-Command uv -ErrorAction SilentlyContinue
if ($uvCmd -and -not $Force) {
    $currentVersion = uv --version 2>&1
    Write-Host "uv already installed: $currentVersion" -ForegroundColor Yellow
    Write-Host "Nothing to do. Use -Force to reinstall."
    exit 0
}

# Method 1: Using standalone installer (recommended)
Write-Host "Installing uv using standalone installer..." -ForegroundColor Cyan
try {
    irm https://astral.sh/uv/install.ps1 | iex
    Write-Host "✓ uv installed via standalone installer!" -ForegroundColor Green
} catch {
    Write-Host "Standalone installation failed. Trying pip method..." -ForegroundColor Yellow
    
    # Method 2: Using pip
    $pythonCmd = Get-Command python -ErrorAction SilentlyContinue
    if (-not $pythonCmd) {
        Write-Host "✗ Python not found. Please install Python first." -ForegroundColor Red
        Write-Host "Run: & `"$scriptDir\install_python.ps1`"" -ForegroundColor Yellow
        exit 1
    }
    
    Write-Host "Installing uv using pip..." -ForegroundColor Cyan
    python -m pip install uv --user
    Write-Host "✓ uv installed via pip!" -ForegroundColor Green
}

# Refresh environment variables
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

# Add cargo bin to PATH if not present
$cargoBinPath = "$env:USERPROFILE\.cargo\bin"
if ($env:Path -notlike "*$cargoBinPath*") {
    Write-Host ""
    Write-Host "Adding $cargoBinPath to PATH..." -ForegroundColor Cyan
    [Environment]::SetEnvironmentVariable(
        "Path",
        $env:Path + ";$cargoBinPath",
        [EnvironmentVariableTarget]::User
    )
    $env:Path += ";$cargoBinPath"
    Write-Host "✓ PATH updated!" -ForegroundColor Green
}

# Verify installation
Write-Host ""
Write-Host "Verifying installation..." -ForegroundColor Cyan
Start-Sleep -Seconds 2

try {
    # Try to find uv in common locations
    $uvPaths = @(
        "$env:USERPROFILE\.cargo\bin\uv.exe",
        "$env:LOCALAPPDATA\Programs\uv\uv.exe"
    )
    
    $uvFound = $false
    foreach ($path in $uvPaths) {
        if (Test-Path $path) {
            $uvVersion = & $path --version 2>&1
            Write-Host "✓ uv: $uvVersion" -ForegroundColor Green
            Write-Host "  Location: $path" -ForegroundColor Gray
            $uvFound = $true
            break
        }
    }
    
    if (-not $uvFound) {
        # Try direct command
        $uvVersion = uv --version 2>&1
        Write-Host "✓ uv: $uvVersion" -ForegroundColor Green
    }
    
    Write-Host ""
    Write-Host "uv installation completed successfully!" -ForegroundColor Green
    Write-Host "Please restart your terminal to ensure PATH is updated." -ForegroundColor Yellow
} catch {
    Write-Host "✗ Verification failed. Please restart your terminal and try again." -ForegroundColor Red
    Write-Host "uv should be in: $cargoBinPath" -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "=== Installation Complete ===" -ForegroundColor Cyan
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "1. Restart your terminal"
Write-Host "2. Run: uv --version"
Write-Host "3. Create venv: uv venv"
Write-Host "4. Install packages: uv pip install package-name"
