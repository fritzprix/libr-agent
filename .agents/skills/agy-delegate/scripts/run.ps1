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

# Resolve agy binary
$agyBin = $env:AGY_BIN
if (-not $agyBin) {
  $cmd = Get-Command agy -ErrorAction SilentlyContinue
  if ($cmd) {
    $agyBin = $cmd.Source
  } elseif (Test-Path "$env:LOCALAPPDATA\agy\bin\agy.exe") {
    $agyBin = "$env:LOCALAPPDATA\agy\bin\agy.exe"
  } elseif (Test-Path "$env:USERPROFILE\.local\bin\agy.exe") {
    $agyBin = "$env:USERPROFILE\.local\bin\agy.exe"
  }
}

if (-not $agyBin -or -not (Test-Path $agyBin)) {
  Write-Error "agy binary not found on PATH or default locations. Set AGY_BIN."
  exit 1
}

$outputFormat = if ($Json) { 'json' } else { 'text' }

$agyArgs = @(
  '-p', $Task,
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

& $agyBin @agyArgs
exit $LASTEXITCODE
