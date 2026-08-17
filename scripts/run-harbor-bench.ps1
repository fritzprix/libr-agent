<#
.SYNOPSIS
  Run Harbor / Terminal-Bench tasks against a live LibrAgent Session API.

.DESCRIPTION
  Requires `pnpm tauri dev` (or any LibrAgent build) with HTTP API enabled.
  Uses benchmarks/harbor/libragent_agent.py (executionMode=unsafe by default).

  Official Terminal-Bench submissions must not modify timeouts or resources.
  Timeout multipliers are omitted by default; pass -TimeoutMultiplier /
  -AgentTimeoutMultiplier (or env vars) only for local debugging.

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
  [ValidateSet("hello", "terminal-bench", "harbor-index", "path", "dataset")]
  [string]$Preset = "hello",

  [string]$Path,

  [string]$Dataset = "terminal-bench/terminal-bench-2-1",

  [string[]]$Include,

  [int]$NTasks = 0,

  # Harbor -k. Official TB 2.1 submissions require at least 5.
  [int]$NAttempts = $(if ($env:LIBRAGENT_N_ATTEMPTS) { [int]$env:LIBRAGENT_N_ATTEMPTS } else { 1 }),

  [int]$Concurrent = 1,

  [string]$ApiUrl = $(if ($env:LIBRAGENT_API_URL) { $env:LIBRAGENT_API_URL } else { "http://localhost:3030/api" }),

  [string]$AssistantId = $env:LIBRAGENT_ASSISTANT_ID,

  # Harbor -m. Prefer this / LIBRAGENT_MODEL; else GET /api/settings/preferredModel.
  [string]$Model = $(
    if ($env:LIBRAGENT_MODEL) { $env:LIBRAGENT_MODEL }
    elseif ($env:LIBRAGENT_HARBOR_MODEL) { $env:LIBRAGENT_HARBOR_MODEL }
    else { $null }
  ),

  [ValidateSet("yolo", "unsafe", "normal")]
  [string]$ExecutionMode = $(if ($env:LIBRAGENT_EXECUTION_MODE) { $env:LIBRAGENT_EXECUTION_MODE } else { "unsafe" }),

  # Omitted by default (submission-compatible). Set via flag or env for local debugging only.
  [Nullable[double]]$TimeoutMultiplier = $(
    if ($env:LIBRAGENT_TIMEOUT_MULTIPLIER) { [double]$env:LIBRAGENT_TIMEOUT_MULTIPLIER } else { $null }
  ),

  [Nullable[double]]$AgentTimeoutMultiplier = $(
    if ($env:LIBRAGENT_AGENT_TIMEOUT_MULTIPLIER) { [double]$env:LIBRAGENT_AGENT_TIMEOUT_MULTIPLIER } else { $null }
  ),

  [int]$SmokeTimeout = $(if ($env:LIBRAGENT_SMOKE_TIMEOUT) { [int]$env:LIBRAGENT_SMOKE_TIMEOUT } else { 300 }),

  [string[]]$VerifierEnv,

  [string]$AssistantName = "Coding Expert",

  [switch]$SkipHealthCheck,

  [switch]$DryRun,

  [switch]$DebugHarbor
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $RepoRoot

# Convenience: if --dataset was explicitly supplied without --preset, treat as dataset preset.
if (-not $PSBoundParameters.ContainsKey('Preset') -and $PSBoundParameters.ContainsKey('Dataset')) {
  $Preset = "dataset"
}

# Avoid Windows cp949 crashes on Harbor's emoji progress UI.
$env:PYTHONUTF8 = "1"
$env:PYTHONIOENCODING = "utf-8"
$env:PYTHONPATH = "$RepoRoot" + $(if ($env:PYTHONPATH) { ";" + $env:PYTHONPATH } else { "" })

function Write-Step([string]$Message) {
  Write-Host "`n==> $Message" -ForegroundColor Cyan
}

function Assert-Command([string]$Name) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    if ($Name -eq "harbor") {
      Write-Step "harbor command not found. Attempting to bootstrap/install..."
      # Install using pip or uv
      if (Get-Command uv -ErrorAction SilentlyContinue) {
        Write-Host "Installing harbor and httpx using uv..."
        uv pip install harbor httpx --system
      } else {
        # Check if python is available first
        if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
          throw "Required command 'python' is not installed, which is needed to install 'harbor'."
        }
        Write-Host "Installing harbor and httpx using pip..."
        python -m pip install harbor httpx
      }
      
      # If still not found in PATH, try to locate Python script directory and add it to PATH
      if (-not (Get-Command harbor -ErrorAction SilentlyContinue)) {
        if (Get-Command python -ErrorAction SilentlyContinue) {
          $pythonScriptDir = python -c "import sysconfig; print(sysconfig.get_path('scripts'))"
          if ($pythonScriptDir -and (Test-Path $pythonScriptDir)) {
            Write-Host "Adding $pythonScriptDir to env:Path for this session" -ForegroundColor Yellow
            $env:Path = "$pythonScriptDir;" + $env:Path
          }
        }
      }
      
      # Final check
      if (-not (Get-Command harbor -ErrorAction SilentlyContinue)) {
        throw "Could not bootstrap harbor. Please install harbor manually (e.g. 'pip install harbor httpx')."
      }
      Write-Step "harbor successfully bootstrapped and ready!"
    } else {
      throw "Required command not found: $Name"
    }
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

function Resolve-HarborModel {
  param(
    [string]$ApiBase,
    [string]$PreferredModel
  )

  if ($PreferredModel -and $PreferredModel.Trim().Length -gt 0) {
    return $PreferredModel.Trim()
  }

  Write-Step "Resolving Harbor -m from LibrAgent global preferredModel"
  $url = "$ApiBase/settings/preferredModel"
  try {
    $payload = Invoke-RestMethod -Uri $url -Method GET -TimeoutSec 15
  } catch {
    throw @"
Failed to read preferredModel from $url : $_
Restart LibrAgent (pnpm tauri dev) so GET /api/settings/preferredModel is available, or pass -Model provider/model.
"@
  }

  $harborModel = [string]$payload.harborModel
  if (-not $harborModel -or $harborModel.Trim().Length -eq 0) {
    $model = [string]$payload.model
    $provider = [string]$payload.provider
    if ($model -and $provider -and ($model -notmatch '/')) {
      $harborModel = "$provider/$model"
    } else {
      $harborModel = $model
    }
  }
  if (-not $harborModel -or $harborModel.Trim().Length -eq 0) {
    throw "preferredModel is empty. Set a preferred model in LibrAgent settings, or pass -Model provider/model."
  }
  Write-Host ("  preferredModel provider={0} model={1}" -f $payload.provider, $payload.model)
  return $harborModel.Trim()
}

function Test-LibrAgentApi {
  param([string]$ApiBase)

  Write-Step "Checking LibrAgent API at $ApiBase/health"
  $health = Invoke-RestMethod -Uri "$ApiBase/health" -Method GET -TimeoutSec 10
  if ($health.status -ne "ok") {
    throw "Unexpected health response: $($health | ConvertTo-Json -Compress)"
  }

  Write-Step "Smoke-checking executionMode=$ExecutionMode (create → verify mode → await idle → delete)"
  # Start a real turn so we verify the session can run, but wait for idle before
  # cleanup — deleting ~500ms into an LLM turn aborts the reply mid-flight.
  $bodyObj = @{
    assistantId         = $script:ResolvedAssistantId
    name                = "harbor-bench-smoke"
    request             = "Reply with exactly: ok"
    executionMode       = $ExecutionMode
    workspaceIsolation  = "host"
  }
  $body = $bodyObj | ConvertTo-Json
  $created = Invoke-RestMethod -Uri "$ApiBase/sessions" -Method POST -Body $body -ContentType "application/json" -TimeoutSec 60
  $sessionId = $created.id
  try {
    $session = Invoke-RestMethod -Uri "$ApiBase/sessions/$sessionId" -Method GET -TimeoutSec 15
    Write-Host ("  session={0} executionMode={1} status={2}" -f $session.id, $session.executionMode, $session.status)
    if ($ExecutionMode -ne "normal" -and $session.executionMode -ne $ExecutionMode) {
      throw "API did not apply executionMode=$ExecutionMode (got '$($session.executionMode)'). Is pnpm tauri dev running a build that includes CreateSessionRequest.executionMode?"
    }
    if ($session.status -eq "idle" -and -not $session.lastMessageAt) {
      throw "Smoke session did not start a workflow (status=idle with no messages). CreateSessionRequest.request may have been ignored."
    }

    $deadline = (Get-Date).AddSeconds($SmokeTimeout)
    while ((Get-Date) -lt $deadline) {
      if ($session.status -in @("idle", "error", "paused")) {
        break
      }
      Start-Sleep -Seconds 1
      $session = Invoke-RestMethod -Uri "$ApiBase/sessions/$sessionId" -Method GET -TimeoutSec 15
    }
    Write-Host ("  settled status={0}" -f $session.status)
    if ($session.status -notin @("idle", "error", "paused")) {
      throw "Smoke session did not settle within ${SmokeTimeout}s (status='$($session.status)')."
    }
  }
  finally {
    try {
      Invoke-RestMethod -Uri "$ApiBase/sessions/$sessionId" -Method DELETE -TimeoutSec 30 | Out-Null
      Write-Host "  smoke session deleted ($sessionId)"
    }
    catch {
      Write-Warning "Failed to delete smoke session ${sessionId}: $_"
    }
  }
}

function Show-LatestRewards {
  param([int]$HarborExitCode = 0)

  $jobsDir = Join-Path $RepoRoot "jobs"
  if (-not (Test-Path $jobsDir)) {
    return
  }
  $latest = Get-ChildItem $jobsDir -Directory | Sort-Object Name -Descending | Select-Object -First 1
  if (-not $latest) {
    return
  }

  Write-Step "Latest job: $($latest.Name)"
  if ($HarborExitCode -ne 0) {
    Write-Host "  (Harbor exited $HarborExitCode; if download/extract failed, this may be a previous job.)" -ForegroundColor Yellow
  }
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

function Test-WindowsLongPathsEnabled {
  try {
    $value = Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -Name LongPathsEnabled -ErrorAction Stop
    return [bool]([int]$value.LongPathsEnabled -eq 1)
  } catch {
    return $false
  }
}

function Get-HarborPython {
  $harborCmd = Get-Command harbor -ErrorAction Stop
  $candidate = Join-Path (Split-Path -Parent $harborCmd.Source) 'python.exe'
  if (Test-Path $candidate) {
    return $candidate
  }
  return (Get-Command python -ErrorAction Stop).Source
}

# Harbor nests packages under ~/.cache/harbor/tasks/packages/... Deep
# harbor-index trees exceed Windows MAX_PATH (~260). On Windows we run Harbor
# via scripts/harbor_short_cache_run.py which patches PACKAGE_CACHE_DIR to a
# short root (default C:\p; override with LIBRAGENT_HARBOR_CACHE).
#
# Do not assign the function output ($x = Invoke-Harbor): native stdout would
# be captured into the return value. Exit code is written to $script:HarborExitCode.
function Invoke-Harbor {
  param([Parameter(Mandatory = $true)][string[]]$HarborArgs)

  $script:HarborExitCode = 0

  if ($env:OS -ne 'Windows_NT') {
    & harbor @HarborArgs
    $script:HarborExitCode = $LASTEXITCODE
    return
  }

  $cacheRoot = if ($env:LIBRAGENT_HARBOR_CACHE) {
    $env:LIBRAGENT_HARBOR_CACHE.TrimEnd('\')
  } else {
    'C:\p'
  }
  $null = New-Item -ItemType Directory -Force -Path $cacheRoot
  $env:LIBRAGENT_HARBOR_CACHE = $cacheRoot

  if (-not (Test-WindowsLongPathsEnabled)) {
    Write-Host ("Windows LongPathsEnabled=0; Harbor package cache -> {0} via harbor_short_cache_run.py (LIBRAGENT_HARBOR_CACHE). Durable fix:" -f $cacheRoot) -ForegroundColor Yellow
    Write-Host '  New-ItemProperty HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem -Name LongPathsEnabled -Value 1 -PropertyType DWORD -Force' -ForegroundColor DarkYellow
  } else {
    Write-Step "Harbor package cache (patched): $cacheRoot"
  }

  $wrapper = Join-Path $PSScriptRoot 'harbor_short_cache_run.py'
  $harborPython = Get-HarborPython
  Write-Step "Running Harbor via $harborPython $wrapper"
  & $harborPython $wrapper @HarborArgs
  $script:HarborExitCode = $LASTEXITCODE
}

Assert-Command "python"
Assert-Command "harbor"

$script:ResolvedAssistantId = Resolve-AssistantId -ApiBase $ApiUrl -PreferredId $AssistantId -NameHint $AssistantName
Write-Host "Using assistantId=$($script:ResolvedAssistantId)"

$script:ResolvedHarborModel = Resolve-HarborModel -ApiBase $ApiUrl -PreferredModel $Model
Write-Host "Using Harbor model (-m)=$($script:ResolvedHarborModel)"

if (-not $SkipHealthCheck) {
  Test-LibrAgentApi -ApiBase $ApiUrl
}

$harborArgs = @(
  "run",
  "-a", "benchmarks.harbor.libragent_agent:LibrAgentHarborAdapter",
  "-m", "$($script:ResolvedHarborModel)",
  "--ak", "api_url=$ApiUrl",
  "--ak", "assistant_id=$($script:ResolvedAssistantId)",
  "--ak", "execution_mode=$ExecutionMode",
  "-n", "$Concurrent",
  "-k", "$NAttempts"
)

if ($null -ne $TimeoutMultiplier) {
  $harborArgs += @("--timeout-multiplier", "$TimeoutMultiplier")
}

if ($null -ne $AgentTimeoutMultiplier) {
  $harborArgs += @("--agent-timeout-multiplier", "$AgentTimeoutMultiplier")
}

if ($VerifierEnv) {
  $envList = $VerifierEnv -split ','
  foreach ($ve in $envList) {
    if ($ve.Trim()) {
      $harborArgs += @("--ve", $ve.Trim())
    }
  }
}

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
      foreach ($pattern in $Include) {
        if ($pattern) {
          $harborArgs += @("-i", $pattern)
        }
      }
    }
    if ($NTasks -gt 0) {
      $harborArgs += @("-l", "$NTasks")
    }
  }
  "harbor-index" {
    if (-not $PSBoundParameters.ContainsKey('Dataset')) {
      $Dataset = "harbor-index/harbor-index-1.0"
    }
    Write-Step "Preset: Harbor Index dataset ($Dataset)"
    $harborArgs += @("-d", $Dataset)
    if ($Include) {
      foreach ($pattern in $Include) {
        if ($pattern) {
          $harborArgs += @("-i", $pattern)
        }
      }
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
      foreach ($pattern in $Include) {
        if ($pattern) {
          $harborArgs += @("-i", $pattern)
        }
      }
    }
    if ($NTasks -gt 0) {
      $harborArgs += @("-l", "$NTasks")
    }
  }
  "dataset" {
    # Default -Dataset is terminal-bench; require an explicit value so bare
    # -Preset dataset does not silently run the wrong registry entry.
    if (-not $PSBoundParameters.ContainsKey('Dataset')) {
      throw "Preset 'dataset' requires -Dataset <org/name-version> (e.g. swe-bench/swe-bench-verified-1.0)"
    }
    Write-Step "Preset: dataset ($Dataset)"
    $harborArgs += @("-d", $Dataset)
    if ($Include) {
      foreach ($pattern in $Include) {
        if ($pattern) {
          $harborArgs += @("-i", $pattern)
        }
      }
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

Invoke-Harbor -HarborArgs $harborArgs
$exitCode = [int]$script:HarborExitCode

Show-LatestRewards -HarborExitCode $exitCode

if ($exitCode -ne 0) {
  Write-Host "`nHarbor exited with code $exitCode." -ForegroundColor Yellow
  Write-Host "If the traceback was FileNotFoundError during tar.extractall, that is usually Windows MAX_PATH (enable LongPathsEnabled, or ensure LIBRAGENT_HARBOR_CACHE is a short path like C:\p)." -ForegroundColor Yellow
  Write-Host "Console emoji encoding can also yield a non-zero exit even when reward.txt=1; trust jobs/<timestamp>/verifier/reward.txt when a job was created." -ForegroundColor Yellow
}

exit $exitCode
