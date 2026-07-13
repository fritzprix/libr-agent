# Python Installation Script for Windows
# Run as Administrator for system-wide installation

param(
    [switch]$UserInstall = $false,
    [string]$Version = "3.12"
)

$ErrorActionPreference = "Stop"

Write-Host "=== Python Installation Script ===" -ForegroundColor Cyan
Write-Host "Target Version: Python $Version" -ForegroundColor Green
Write-Host ""

# Check if Python is already installed
$pythonCmd = Get-Command python -ErrorAction SilentlyContinue
if ($pythonCmd) {
    $currentVersion = python --version 2>&1
    Write-Host "Python already installed: $currentVersion" -ForegroundColor Yellow
    $continue = Read-Host "Do you want to continue with installation? (y/n)"
    if ($continue -ne 'y') {
        Write-Host "Installation cancelled." -ForegroundColor Red
        exit 0
    }
}

# Install using winget (recommended)
Write-Host "Installing Python using winget..." -ForegroundColor Cyan
try {
    if ($UserInstall) {
        winget install --id Python.Python.3.12 --scope user --silent
    } else {
        winget install --id Python.Python.3.12 --scope machine --silent
    }
    Write-Host "Python installed successfully!" -ForegroundColor Green
} catch {
    Write-Host "winget installation failed. Trying alternative method..." -ForegroundColor Yellow
    
    # Fallback: Download and install manually
    $downloadUrl = "https://www.python.org/ftp/python/3.12.0/python-3.12.0-amd64.exe"
    $installerPath = "$env:TEMP\python-installer.exe"
    
    Write-Host "Downloading Python installer..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $downloadUrl -OutFile $installerPath
    
    Write-Host "Running installer..." -ForegroundColor Cyan
    if ($UserInstall) {
        Start-Process -FilePath $installerPath -ArgumentList "/quiet", "InstallAllUsers=0", "PrependPath=1" -Wait
    } else {
        Start-Process -FilePath $installerPath -ArgumentList "/quiet", "InstallAllUsers=1", "PrependPath=1" -Wait
    }
    
    Remove-Item $installerPath -Force
    Write-Host "Python installed successfully!" -ForegroundColor Green
}

# Refresh environment variables
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

# Verify installation
Write-Host ""
Write-Host "Verifying installation..." -ForegroundColor Cyan
Start-Sleep -Seconds 2

try {
    $pythonVersion = python --version 2>&1
    $pipVersion = pip --version 2>&1
    
    Write-Host "✓ Python: $pythonVersion" -ForegroundColor Green
    Write-Host "✓ pip: $pipVersion" -ForegroundColor Green
    
    Write-Host ""
    Write-Host "Python installation completed successfully!" -ForegroundColor Green
    Write-Host "Please restart your terminal to ensure PATH is updated." -ForegroundColor Yellow
} catch {
    Write-Host "✗ Verification failed. Please restart your terminal and try again." -ForegroundColor Red
    Write-Host "If the problem persists, check PATH configuration." -ForegroundColor Yellow
    exit 1
}

# Upgrade pip
Write-Host ""
Write-Host "Upgrading pip to latest version..." -ForegroundColor Cyan
try {
    python -m pip install --upgrade pip
    Write-Host "✓ pip upgraded successfully!" -ForegroundColor Green
} catch {
    Write-Host "⚠ Failed to upgrade pip. You may need to upgrade manually later." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "=== Installation Complete ===" -ForegroundColor Cyan
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "1. Restart your terminal (or reboot if needed)"
Write-Host "2. Run: python --version"
Write-Host "3. Install uv: pip install uv"
