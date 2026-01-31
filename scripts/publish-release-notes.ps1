#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Publishes changelog content to GitHub releases
.DESCRIPTION
    Extracts changelog content from CHANGELOG.md and creates/updates a GitHub release
.PARAMETER Version
    The version to publish (e.g., 0.4.6)
.EXAMPLE
    .\publish-release-notes.ps1 0.4.6
#>

param(
    [Parameter(Mandatory=$true)]
    [string]$Version
)

$ErrorActionPreference = "Stop"

$TAG = "v$Version"

Write-Host ">>> Publishing release notes for $TAG..." -ForegroundColor Cyan

# Check if tag exists
try {
    git rev-parse $TAG 2>&1 | Out-Null
} catch {
    Write-Host "Error: Tag $TAG does not exist" -ForegroundColor Red
    Write-Host "Create the tag first: git tag $TAG" -ForegroundColor Yellow
    exit 1
}

# Check for gh CLI
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    Write-Host "Error: GitHub CLI (gh) is not installed." -ForegroundColor Red
    Write-Host "Install it from: https://cli.github.com/" -ForegroundColor Yellow
    exit 1
}

# Extract changelog content
$changelogPath = Join-Path $PSScriptRoot ".." "CHANGELOG.md"
$changelogContent = Get-Content $changelogPath -Raw

# Extract content for this version or Unreleased
$pattern = "(?s)## \[(Unreleased|$Version)\](.*?)(?=\n## \[|\z)"
if ($changelogContent -match $pattern) {
    $releaseNotes = $Matches[2].Trim()
    
    if ([string]::IsNullOrWhiteSpace($releaseNotes)) {
        Write-Host "Error: No changelog content found for version $Version" -ForegroundColor Red
        Write-Host "Please update CHANGELOG.md with a section for [$Version]" -ForegroundColor Yellow
        exit 1
    }
} else {
    Write-Host "Error: No changelog section found for version $Version" -ForegroundColor Red
    Write-Host "Please update CHANGELOG.md with a section for [$Version] or [Unreleased]" -ForegroundColor Yellow
    exit 1
}

Write-Host "`nExtracted release notes ($($releaseNotes.Length) characters)" -ForegroundColor Green

# Check if release already exists
$releaseExists = $false
try {
    gh release view $TAG 2>&1 | Out-Null
    $releaseExists = $true
} catch {
    $releaseExists = $false
}

if ($releaseExists) {
    Write-Host "Release $TAG already exists." -ForegroundColor Yellow
    $response = Read-Host "Do you want to update the release notes? (y/n)"
    
    if ($response -eq 'y' -or $response -eq 'Y') {
        # Save notes to temp file (gh CLI doesn't accept piped input reliably on Windows)
        $tempFile = [System.IO.Path]::GetTempFileName()
        $releaseNotes | Out-File -FilePath $tempFile -Encoding UTF8 -NoNewline
        
        try {
            gh release edit $TAG --notes-file $tempFile
            Write-Host "✓ Release notes updated for $TAG" -ForegroundColor Green
        } finally {
            Remove-Item $tempFile -ErrorAction SilentlyContinue
        }
    }
} else {
    # Create new draft release
    Write-Host "Creating new draft release..." -ForegroundColor Cyan
    
    $tempFile = [System.IO.Path]::GetTempFileName()
    $releaseNotes | Out-File -FilePath $tempFile -Encoding UTF8 -NoNewline
    
    try {
        gh release create $TAG `
            --title "Release $TAG" `
            --notes-file $tempFile `
            --draft `
            --verify-tag
        
        Write-Host "✓ Draft release created: https://github.com/fritzprix/libr-agent/releases/tag/$TAG" -ForegroundColor Green
        Write-Host "  Please review and publish the release manually on GitHub." -ForegroundColor Yellow
    } finally {
        Remove-Item $tempFile -ErrorAction SilentlyContinue
    }
}

Write-Host "`n>>> Done!" -ForegroundColor Cyan
