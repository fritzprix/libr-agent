<#
.SYNOPSIS
  Run Harbor / Terminal-Bench tasks against a live LibrAgent Session API.

.DESCRIPTION
  Requires `pnpm tauri dev` (or any LibrAgent build) with HTTP API enabled.
  Uses benchmarks/harbor/libragent_agent.py (executionMode=yolo by default).

.EXAMPLE
  # Smoke task (hello-world)
  .\scripts\run-harbor-bench.ps1 -Preset hello

.EXAMPLE
  # Terminal-Bench: first task only
  .\scripts\run-harbor-bench.ps1 -Preset terminal-bench -NTasks 1

.EXAMPLE
  # Terminal-Bench: one named task
  .\scripts\run-harbor-bench.ps1 -Preset terminal-bench -Include "hello-world*"

.EXAMPLE
  # Local task directory
  .\scripts\run-harbor-bench.ps1 -Preset path -Path C:\path\to\task
#>
[CmdletBinding()]
param(
  [ValidateSet("hello", "terminal-bench", "path")]
  [string]$Preset = "hello",

  [string]$Path,

  [string]$Dataset = "terminal-bench@2.0",

  [string]$Include,

  [int]$NTasks = 0,

  [int]$Concurrent = 1,

  [string]$ApiUrl = $(if ($env:LIBRAGENT_API_URL) { $env:LIBRAGENT_API_URL } else { "http://localhost:3030/api" }),

  [string]$AssistantId = $env:LIBRAGENT_ASSISTANT_ID,

  [ValidateSet("yolo", "unsafe", "normal")]
  [string]$ExecutionMode = "yolo",

  [string]$AssistantName = "Coding Expert",

  [switch]$SkipHealthCheck,

  [switch]$DryRun,

  [switch]$DebugHarbor
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $RepoRoot

# Avoid Windows cp949 crashes on Harbor's emoji progress UI.
$env:PYTHONUTF8 = "1"
$env:PYTHONIOENCODING = "utf-8"
$env:PYTHONPATH = "$RepoRoot" + $(if ($env:PYTHONPATH) { ";" + $env:PYTHONPATH } else { "" })

function Write-Step([string]$Message) {
  Write-Host "`n==> $Message" -ForegroundColor Cyan
}

function Assert-Command([string]$Name) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Required command not found: $Name"
  }
}

function Resolve-AssistantId {
  param([string]$ApiBase, [string]$PreferredId, [string]$NameHint)

  if ($PreferredId -and $PreferredId.Trim().Length -gt 0) {
    return $PreferredId.Trim()
  }

  Write-Step "Resolving assistant '$NameHint' from $ApiBase/assistants"
  $payload = Invoke-RestMethod -Uri "$ApiBase/assistants" -Method GET -TimeoutSec 15
  $assistants = @()
  if ($payload -is [System.Array]) {
    $assistants = $payload
  } elseif ($payload.assistants) {
    $assistants = @($payload.assistants)
  }

  $match = $assistants | Where-Object { $_.name -eq $NameHint } | Select-Object -First 1
  if (-not $match) {
    $match = $assistants | Where-Object { $_.name -like "*Coding*" } | Select-Object -First 1
  }
  if (-not $match) {
    throw "Could not resolve assistant id. Pass -AssistantId or set LIBRAGENT_ASSISTANT_ID."
  }
  return [string]$match.id
}

function Test-LibrAgentApi {
  param([string]$ApiBase)

  Write-Step "Checking LibrAgent API at $ApiBase/health"
  $health = Invoke-RestMethod -Uri "$ApiBase/health" -Method GET -TimeoutSec 10
  if ($health.status -ne "ok") {
    throw "Unexpected health response: $($health | ConvertTo-Json -Compress)"
  }

  Write-Step "Smoke-checking executionMode=$ExecutionMode"
  $bodyObj = @{
    assistantId         = $script:ResolvedAssistantId
    name                = "harbor-bench-smoke"
    request             = "Reply with exactly: ok"
    executionMode       = $ExecutionMode
    workspaceIsolation  = "host"
  }
  $body = $bodyObj | ConvertTo-Json
  $created = Invoke-RestMethod -Uri "$ApiBase/sessions" -Method POST -Body $body -ContentType "application/json" -TimeoutSec 60
  Start-Sleep -Seconds 1
  $session = Invoke-RestMethod -Uri "$ApiBase/sessions/$($created.id)" -Method GET -TimeoutSec 15
  Write-Host ("  session={0} executionMode={1} status={2}" -f $session.id, $session.executionMode, $session.status)
  if ($ExecutionMode -ne "normal" -and $session.executionMode -ne $ExecutionMode) {
    throw "API did not apply executionMode=$ExecutionMode (got '$($session.executionMode)'). Is pnpm tauri dev running a build that includes the CreateSessionRequest change?"
  }
}

function Show-LatestRewards {
  $jobsDir = Join-Path $RepoRoot "jobs"
  if (-not (Test-Path $jobsDir)) {
    return
  }
  $latest = Get-ChildItem $jobsDir -Directory | Sort-Object Name -Descending | Select-Object -First 1
  if (-not $latest) {
    return
  }

  Write-Step "Latest job: $($latest.Name)"
  $resultJson = Join-Path $latest.FullName "result.json"
  if (Test-Path $resultJson) {
    try {
      $job = Get-Content $resultJson -Raw | ConvertFrom-Json
      if ($job.stats -and $job.stats.evals) {
        $job.stats.evals.PSObject.Properties | ForEach-Object {
          $metrics = $_.Value.metrics
          $mean = $null
          if ($metrics -and $metrics.Count -gt 0) {
            $mean = $metrics[0].mean
          }
          Write-Host ("  eval {0}: mean={1} trials={2} errors={3}" -f $_.Name, $mean, $_.Value.n_trials, $_.Value.n_errors)
        }
      }
    } catch {
      Write-Host "  (could not parse job result.json)"
    }
  }

  Get-ChildItem $latest.FullName -Directory | ForEach-Object {
    $rewardFile = Join-Path $_.FullName "verifier\reward.txt"
    if (Test-Path $rewardFile) {
      $reward = (Get-Content $rewardFile -Raw).Trim()
      Write-Host ("  trial {0}: reward={1}" -f $_.Name, $reward)
    }
  }
}

Assert-Command "harbor"
Assert-Command "python"

$script:ResolvedAssistantId = Resolve-AssistantId -ApiBase $ApiUrl -PreferredId $AssistantId -NameHint $AssistantName
Write-Host "Using assistantId=$($script:ResolvedAssistantId)"

if (-not $SkipHealthCheck) {
  Test-LibrAgentApi -ApiBase $ApiUrl
}

$harborArgs = @(
  "run",
  "-a", "benchmarks.harbor.libragent_agent:LibrAgentHarborAdapter",
  "--ak", "api_url=$ApiUrl",
  "--ak", "assistant_id=$($script:ResolvedAssistantId)",
  "--ak", "execution_mode=$ExecutionMode",
  "-n", "$Concurrent"
)

switch ($Preset) {
  "hello" {
    Write-Step "Preset: hello-world (Harbor example)"
    $cached = Join-Path $env:USERPROFILE ".cache\harbor\tasks"
    $cachedHello = $null
    if (Test-Path $cached) {
      $cachedHello = Get-ChildItem $cached -Directory -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -eq "hello-world" } |
        Select-Object -First 1 -ExpandProperty FullName
    }
    if ($cachedHello) {
      Write-Host "  Using cached task: $cachedHello"
      $harborArgs += @("-p", $cachedHello)
    } else {
      $harborArgs += @(
        "--task-git-url", "https://github.com/laude-institute/harbor.git",
        "-p", "examples/tasks/hello-world"
      )
    }
  }
  "terminal-bench" {
    Write-Step "Preset: Terminal-Bench dataset ($Dataset)"
    $harborArgs += @("-d", $Dataset)
    if ($Include) {
      $harborArgs += @("-i", $Include)
    }
    if ($NTasks -gt 0) {
      $harborArgs += @("-l", "$NTasks")
    }
  }
  "path" {
    if (-not $Path) {
      throw "Preset 'path' requires -Path <task-or-dataset-dir>"
    }
    $resolvedPath = (Resolve-Path $Path).Path
    Write-Step "Preset: local path ($resolvedPath)"
    $harborArgs += @("-p", $resolvedPath)
    if ($Include) {
      $harborArgs += @("-i", $Include)
    }
    if ($NTasks -gt 0) {
      $harborArgs += @("-l", "$NTasks")
    }
  }
}

if ($DebugHarbor) {
  $harborArgs += @("--debug")
}

Write-Step "Running: harbor $($harborArgs -join ' ')"
if ($DryRun) {
  Write-Host "Dry run only; not executing."
  exit 0
}

& harbor @harborArgs
$exitCode = $LASTEXITCODE

Show-LatestRewards

if ($exitCode -ne 0) {
  Write-Host "`nHarbor exited with code $exitCode (Windows console encoding issues can still leave reward.txt=1)." -ForegroundColor Yellow
}

exit $exitCode
