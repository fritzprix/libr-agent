# Python Installation Script for Windows
# Non-interactive and user scope: no prompts and no administrator rights.
# Order: uv-managed Python (`uv python install`), then winget --scope user.

param(
    [string]$Version = "3.12",
    [switch]$Force = $false
)

$ErrorActionPreference = "Continue"

Write-Host "=== Python Installation Script ===" -ForegroundColor Cyan
Write-Host "Target Version: Python $Version" -ForegroundColor Green
Write-Host ""

function Test-RedirectionGuard($text) {
    return ($text -match 'os error 448|WinError 448|ERROR_UNTRUSTED_MOUNT_POINT|untrusted mount point')
}

function Stop-RedirectionGuard($text) {
    Write-Host "✗ Windows RedirectionGuard blocked the uv Python junction (os error 448, ERROR_UNTRUSTED_MOUNT_POINT)." -ForegroundColor Red
    Write-Host "  $text"
    Write-Host "NEXT: Report this error to the user. Do not reinstall." -ForegroundColor Yellow
    exit 1
}

# A working Python is enough; the WindowsApps alias fails to run, so it does not count.
$pythonCmd = Get-Command python -ErrorAction SilentlyContinue
if ($pythonCmd -and -not $Force) {
    $currentVersion = (& python --version 2>&1) -join " "
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Python already installed: $currentVersion" -ForegroundColor Yellow
        Write-Host "Nothing to do. Use -Force to install Python $Version anyway."
        exit 0
    }
}

$installed = $false

# 1. uv-managed Python (user scope, no PATH change needed: use `uv run --python`).
if (Get-Command uv -ErrorAction SilentlyContinue) {
    Write-Host "Installing Python $Version with uv..." -ForegroundColor Cyan
    $uvOutput = (& uv python install $Version 2>&1) -join "`n"
    $uvExit = $LASTEXITCODE
    if (Test-RedirectionGuard $uvOutput) { Stop-RedirectionGuard $uvOutput }
    if ($uvExit -eq 0) {
        $found = (& uv python find $Version 2>&1) -join "`n"
        $findExit = $LASTEXITCODE
        if (Test-RedirectionGuard $found) { Stop-RedirectionGuard $found }
        if ($findExit -eq 0) {
            Write-Host "✓ Python $Version (uv): $found" -ForegroundColor Green
            Write-Host "Run scripts with: uv run --python $Version <script.py>"
            $installed = $true
        }
    } else {
        Write-Host "uv python install failed: $uvOutput" -ForegroundColor Yellow
    }
}

# 2. winget, user scope only.
if (-not $installed -and (Get-Command winget -ErrorAction SilentlyContinue)) {
    Write-Host "Installing Python $Version with winget (user scope)..." -ForegroundColor Cyan
    & winget install --id "Python.Python.$Version" --scope user --silent `
        --accept-package-agreements --accept-source-agreements
    if ($LASTEXITCODE -eq 0) {
        $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
            [System.Environment]::GetEnvironmentVariable("Path", "User")
        $pythonVersion = (& python --version 2>&1) -join " "
        if ($LASTEXITCODE -eq 0) {
            Write-Host "✓ Python: $pythonVersion" -ForegroundColor Green
            $installed = $true
        }
    }
}

if (-not $installed) {
    Write-Host "✗ Python $Version could not be installed without administrator rights." -ForegroundColor Red
    Write-Host "NEXT: Report this to the user. Ask them to install uv (install_uv.ps1) or Python $Version, then run verify_setup.ps1 again." -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "=== Installation Complete ===" -ForegroundColor Cyan
Write-Host "Next step: run verify_setup.ps1 to confirm."
exit 0
