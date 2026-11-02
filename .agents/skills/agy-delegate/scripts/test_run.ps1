[CmdletBinding()]
param(
  [string]$Model = 'gemini-3.8-flash-high'
)

$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$runPs1 = Join-Path $scriptDir 'run.ps1'

$testPrompt = @'
Reply with EXACTLY: "items-center" and `code` and 한국어. Do not add any extra words.
'@

Write-Host "Running agy-delegate test prompt with double quotes, backticks, and Korean text..."
$output = & $runPs1 -Task $testPrompt -Model $Model -Timeout '60s'
Write-Host "Output: $output"

if ($output -notmatch 'items-center' -or $output -notmatch 'code' -or $output -notmatch '한국어') {
  Write-Error "Test prompt failed: output did not contain expected strings."
  exit 1
}

Write-Host "Test prompt passed successfully."
