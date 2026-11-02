<#
.SYNOPSIS
  PowerShell wrapper for agy print-mode delegation (Windows counterpart to run.sh).

.DESCRIPTION
  Safely executes agy CLI in print mode with proper argument quoting,
  bypassing PowerShell argument-splitting traps.

.EXAMPLE
  .\run.ps1 -Task "Audit code changes" -Dir "C:\path\to\repo"
  .\run.ps1 -Json -Dir "C:\path\to\repo" -Task "Run review"
#>

[CmdletBinding()]
param(
  [Parameter(Mandatory = $true, Position = 0)]
  [string]$Task,

  [string]$Model = $env:AGY_MODEL,

  [string]$Timeout = $env:AGY_TIMEOUT,

  [string[]]$Dir = @(),

  [switch]$Json,

  [switch]$NoSkipPerms
)

$ErrorActionPreference = 'Stop'

if (-not $Model) {
  $Model = 'gemini-3.8-flash-high'
}
if (-not $Timeout) {
  $Timeout = '300s'
}

# Ensure agy directory is on PATH if in standard install locations
if ($env:AGY_BIN) {
  $customDir = Split-Path -Parent (Resolve-Path $env:AGY_BIN)
  if ($customDir) {
    $env:PATH = "$customDir;$env:PATH"
  }
}

if (-not (Get-Command agy -ErrorAction SilentlyContinue)) {
  if (Test-Path "$env:LOCALAPPDATA\agy\bin\agy.exe") {
    $env:PATH = "$env:LOCALAPPDATA\agy\bin;$env:PATH"
  } elseif (Test-Path "$env:USERPROFILE\.local\bin\agy.exe") {
    $env:PATH = "$env:USERPROFILE\.local\bin;$env:PATH"
  }
}

if (-not (Get-Command agy -ErrorAction SilentlyContinue)) {
  Write-Error "agy binary not found on PATH or default locations. Set AGY_BIN."
  exit 1
}

$outputFormat = if ($Json) { 'json' } else { 'text' }

$agyArgs = @(
  '--model', $Model,
  '--print-timeout', $Timeout,
  '--output-format', $outputFormat
)

if (-not $NoSkipPerms) {
  $agyArgs += '--dangerously-skip-permissions'
}

foreach ($d in $Dir) {
  if ($d) {
    $agyArgs += @('--add-dir', $d)
  }
}

# Strip leading UTF-8 BOM if present on $Task
if ($Task -and $Task.Length -gt 0 -and [int]$Task[0] -eq 0xFEFF) {
  $Task = $Task.Substring(1)
}

$prevOutputEncoding = $OutputEncoding
$prevConsoleOutputEncoding = [Console]::OutputEncoding
try {
  $utf8 = [System.Text.UTF8Encoding]::new($false)
  $OutputEncoding = $utf8
  [Console]::OutputEncoding = $utf8
  $Task | agy @agyArgs
} finally {
  $OutputEncoding = $prevOutputEncoding
  [Console]::OutputEncoding = $prevConsoleOutputEncoding
}
exit $LASTEXITCODE
