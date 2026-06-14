# System Setup Verification Script for Windows
# Checks Python, Node.js, and uv installations

$ErrorActionPreference = "Continue"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  System Setup Verification for MCP" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$allGood = $true

# Check Python
Write-Host "[1/3] Checking Python..." -ForegroundColor Yellow
$pythonCmd = Get-Command python -ErrorAction SilentlyContinue
if ($pythonCmd) {
    try {
        $pythonVersion = python --version 2>&1
        $pipVersion = pip --version 2>&1
        
        Write-Host "  ✓ Python: $pythonVersion" -ForegroundColor Green
        Write-Host "  ✓ pip: $pipVersion" -ForegroundColor Green
        
        # Check Python version (3.11+)
        if ($pythonVersion -match "Python (\d+)\.(\d+)") {
            $major = [int]$matches[1]
            $minor = [int]$matches[2]
            if ($major -lt 3 -or ($major -eq 3 -and $minor -lt 11)) {
                Write-Host "  ⚠ Warning: Python 3.11+ recommended (found $major.$minor)" -ForegroundColor Yellow
            }
        }
    } catch {
        Write-Host "  ✗ Python found but not working properly" -ForegroundColor Red
        $allGood = $false
    }
} else {
    Write-Host "  ✗ Python not found" -ForegroundColor Red
    Write-Host "    Install with: .\scripts\install_python.ps1" -ForegroundColor Gray
    $allGood = $false
}

Write-Host ""

# Check Node.js
Write-Host "[2/3] Checking Node.js..." -ForegroundColor Yellow
$nodeCmd = Get-Command node -ErrorAction SilentlyContinue
if ($nodeCmd) {
    try {
        $nodeVersion = node --version 2>&1
        $npmVersion = npm --version 2>&1
        
        Write-Host "  ✓ Node.js: $nodeVersion" -ForegroundColor Green
        Write-Host "  ✓ npm: $npmVersion" -ForegroundColor Green
        
        # Check Node version (18+)
        if ($nodeVersion -match "v(\d+)\.") {
            $major = [int]$matches[1]
            if ($major -lt 18) {
                Write-Host "  ⚠ Warning: Node.js 18+ recommended (found v$major)" -ForegroundColor Yellow
            }
        }
    } catch {
        Write-Host "  ✗ Node.js found but not working properly" -ForegroundColor Red
        $allGood = $false
    }
} else {
    Write-Host "  ✗ Node.js not found" -ForegroundColor Red
    Write-Host "    Install with: .\scripts\install_node.ps1" -ForegroundColor Gray
    $allGood = $false
}

Write-Host ""

# Check uv
Write-Host "[3/3] Checking uv..." -ForegroundColor Yellow
$uvCmd = Get-Command uv -ErrorAction SilentlyContinue
if ($uvCmd) {
    try {
        $uvVersion = uv --version 2>&1
        Write-Host "  ✓ uv: $uvVersion" -ForegroundColor Green
    } catch {
        Write-Host "  ✗ uv found but not working properly" -ForegroundColor Red
        $allGood = $false
    }
} else {
    Write-Host "  ✗ uv not found" -ForegroundColor Red
    Write-Host "    Install with: .\scripts\install_uv.ps1" -ForegroundColor Gray
    $allGood = $false
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan

# Summary
if ($allGood) {
    Write-Host "✓ All systems ready for MCP!" -ForegroundColor Green
    Write-Host ""
    Write-Host "You can now:" -ForegroundColor Cyan
    Write-Host "  • Run Python-based MCP servers"
    Write-Host "  • Run Node.js-based MCP servers"
    Write-Host "  • Use uv for fast Python package management"
    Write-Host ""
} else {
    Write-Host "✗ Some components are missing" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please install missing components:" -ForegroundColor Yellow
    Write-Host "  • Python:  .\scripts\install_python.ps1"
    Write-Host "  • Node.js: .\scripts\install_node.ps1"
    Write-Host "  • uv:      .\scripts\install_uv.ps1"
    Write-Host ""
    Write-Host "Or install all at once:" -ForegroundColor Yellow
    Write-Host "  .\scripts\install_python.ps1; .\scripts\install_node.ps1; .\scripts\install_uv.ps1"
    Write-Host ""
}

# PATH check
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "PATH Configuration:" -ForegroundColor Cyan
Write-Host ""

$pathEntries = $env:PATH -split ';'
$relevantPaths = $pathEntries | Where-Object {
    $_ -match 'Python|node|npm|cargo|\.local'
}

if ($relevantPaths) {
    Write-Host "Relevant PATH entries:" -ForegroundColor Green
    foreach ($path in $relevantPaths) {
        Write-Host "  • $path" -ForegroundColor Gray
    }
} else {
    Write-Host "⚠ No Python/Node paths found in PATH" -ForegroundColor Yellow
    Write-Host "You may need to restart your terminal or reboot." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan

# Exit with appropriate code
if ($allGood) {
    exit 0
} else {
    exit 1
}
