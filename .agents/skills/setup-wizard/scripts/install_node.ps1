# Node.js Installation Script for Windows
# Run as Administrator for system-wide installation

param(
    [switch]$UserInstall = $false,
    [string]$Version = "LTS"
)

$ErrorActionPreference = "Stop"

Write-Host "=== Node.js Installation Script ===" -ForegroundColor Cyan
Write-Host "Target Version: $Version" -ForegroundColor Green
Write-Host ""

# Check if Node.js is already installed
$nodeCmd = Get-Command node -ErrorAction SilentlyContinue
if ($nodeCmd) {
    $currentVersion = node --version 2>&1
    Write-Host "Node.js already installed: $currentVersion" -ForegroundColor Yellow
    $continue = Read-Host "Do you want to continue with installation? (y/n)"
    if ($continue -ne 'y') {
        Write-Host "Installation cancelled." -ForegroundColor Red
        exit 0
    }
}

# Install using winget (recommended)
Write-Host "Installing Node.js using winget..." -ForegroundColor Cyan
try {
    if ($UserInstall) {
        winget install --id OpenJS.NodeJS.LTS --scope user --silent
    } else {
        winget install --id OpenJS.NodeJS.LTS --scope machine --silent
    }
    Write-Host "Node.js installed successfully!" -ForegroundColor Green
} catch {
    Write-Host "winget installation failed. Please install manually from nodejs.org" -ForegroundColor Red
    Write-Host "Download: https://nodejs.org/en/download/" -ForegroundColor Yellow
    exit 1
}

# Refresh environment variables
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

# Verify installation
Write-Host ""
Write-Host "Verifying installation..." -ForegroundColor Cyan
Start-Sleep -Seconds 2

try {
    $nodeVersion = node --version 2>&1
    $npmVersion = npm --version 2>&1
    
    Write-Host "✓ Node.js: $nodeVersion" -ForegroundColor Green
    Write-Host "✓ npm: $npmVersion" -ForegroundColor Green
    
    Write-Host ""
    Write-Host "Node.js installation completed successfully!" -ForegroundColor Green
    Write-Host "Please restart your terminal to ensure PATH is updated." -ForegroundColor Yellow
} catch {
    Write-Host "✗ Verification failed. Please restart your terminal and try again." -ForegroundColor Red
    exit 1
}

# Configure npm global directory (user scope)
if ($UserInstall) {
    Write-Host ""
    Write-Host "Configuring npm global directory..." -ForegroundColor Cyan
    $npmPrefix = "$env:APPDATA\npm"
    npm config set prefix $npmPrefix
    Write-Host "✓ npm global directory: $npmPrefix" -ForegroundColor Green
    Write-Host "Ensure $npmPrefix is in your PATH" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "=== Installation Complete ===" -ForegroundColor Cyan
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "1. Restart your terminal"
Write-Host "2. Run: node --version"
Write-Host "3. Run: npm --version"
