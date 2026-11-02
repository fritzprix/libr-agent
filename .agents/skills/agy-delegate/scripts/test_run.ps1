[CmdletBinding()]
param(
  [string]$Model = 'gemini-3.8-flash-high'
)

$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$runPs1 = Join-Path $scriptDir 'run.ps1'

$koreanWord = -join [char[]]@(0xD55C, 0xAD6D, 0xC5B4)
$testPrompt = "Reply with EXACTLY: `"items-center`" and ``code`` and $koreanWord. Do not add any extra words."

Write-Host "Running agy-delegate test prompt with double quotes, backticks, and Korean text..."
$output = & $runPs1 -Task $testPrompt -Model $Model -Timeout '60s'
$exitCode = $LASTEXITCODE

if ($exitCode -ne 0) {
  Write-Error "Test prompt failed: run.ps1 exited with code $exitCode"
  exit $exitCode
}

$text = ($output -join "`n").Trim()
Write-Host "Output: $text"

if (-not $text) {
  Write-Error "Test prompt failed: output was empty."
  exit 1
}

if ($text -notmatch 'items-center' -or $text -notmatch 'code' -or $text -notmatch $koreanWord) {
  Write-Error "Test prompt failed: output did not contain expected strings."
  exit 1
}

Write-Host "Test prompt passed successfully."
