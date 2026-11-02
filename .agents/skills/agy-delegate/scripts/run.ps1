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
$prevErrorActionPreference = $ErrorActionPreference
$exitCode = 0
try {
  $utf8 = [System.Text.UTF8Encoding]::new($false)
  $OutputEncoding = $utf8
  [Console]::OutputEncoding = $utf8
  # Allow native command stderr redirection without PowerShell NativeCommandError termination
  $ErrorActionPreference = 'Continue'

  # Run agy with piped stdin.
  # If agy exits 2 with "unexpected argument ... agy.exe" due to a daemon restart glitch,
  # retry once with the same prompt.
  $attempt = 1
  while ($true) {
    $errFile = [System.IO.Path]::GetTempFileName()
    try {
      $Task | agy @agyArgs 2>$errFile
      $exitCode = $LASTEXITCODE

      $errText = ""
      if (Test-Path $errFile) {
        $errText = [System.IO.File]::ReadAllText($errFile)
      }

      $isAgyArgGlitch = ($exitCode -eq 2) -and ($errText -match 'unexpected argument.*agy(?:\.exe)?')
      if ($isAgyArgGlitch -and $attempt -eq 1) {
        $attempt++
        continue
      }

      if ($errText) {
        [Console]::Error.Write($errText)
      }
      break
    } finally {
      if (Test-Path $errFile) {
        Remove-Item -Force $errFile -ErrorAction SilentlyContinue
      }
    }
  }
} finally {
  $OutputEncoding = $prevOutputEncoding
  [Console]::OutputEncoding = $prevConsoleOutputEncoding
  $ErrorActionPreference = $prevErrorActionPreference
}
exit $exitCode
