[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$DoctorArgs
)

$ErrorActionPreference = 'Continue'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$convertPy = Join-Path $scriptDir 'convert.py'

function Test-PythonExecutable($exePath, $extraArgs = @()) {
    if (-not $exePath) { return $false }
    try {
        $null = & $exePath @extraArgs -c "import sys; sys.exit(0)" 2>$null
        return ($LASTEXITCODE -eq 0)
    } catch {
        return $false
    }
}

$workingRunner = $null
$runnerArgs = @()

# 1. uv run
if (Get-Command uv -ErrorAction SilentlyContinue) {
    if (Test-PythonExecutable 'uv' @('run', '--no-project', '-q', 'python')) {
        $workingRunner = 'uv'
        $runnerArgs = @('run', '--no-project', 'python')
    }
}

# 2. py launcher
if (-not $workingRunner -and (Get-Command py -ErrorAction SilentlyContinue)) {
    if (Test-PythonExecutable 'py' @('-3')) {
        $workingRunner = 'py'
        $runnerArgs = @('-3')
    }
}

# 3. User / System Python installations
if (-not $workingRunner) {
    $searchPatterns = @()
    if ($env:LOCALAPPDATA) {
        $searchPatterns += "$env:LOCALAPPDATA\Programs\Python\Python3*\python.exe"
        $searchPatterns += "$env:LOCALAPPDATA\uv\python\*\python.exe"
    }
    if ($env:APPDATA) {
        $searchPatterns += "$env:APPDATA\uv\python\*\python.exe"
    }
    if ($env:ProgramFiles) {
        $searchPatterns += "$env:ProgramFiles\Python3*\python.exe"
    }
    if ($env:USERPROFILE) {
        $searchPatterns += "$env:USERPROFILE\.local\bin\python.exe"
    }

    foreach ($pat in $searchPatterns) {
        $resolved = Resolve-Path $pat -ErrorAction SilentlyContinue
        foreach ($m in $resolved) {
            $candidate = $m.Path
            if (Test-PythonExecutable $candidate) {
                $workingRunner = $candidate
                $runnerArgs = @()
                break
            }
        }
        if ($workingRunner) { break }
    }
}

# 4. PATH python (only if functional)
if (-not $workingRunner) {
    foreach ($cmdName in @('python3', 'python')) {
        $cmdObj = Get-Command $cmdName -ErrorAction SilentlyContinue
        if ($cmdObj) {
            if (Test-PythonExecutable $cmdObj.Source) {
                $workingRunner = $cmdObj.Source
                $runnerArgs = @()
                break
            }
        }
    }
}

if (-not $workingRunner) {
    [Console]::Error.WriteLine("Error: No functional Python interpreter found on the system.")
    [Console]::Out.WriteLine("ExitCode: 1")
    [Console]::Error.WriteLine("NEXT: Install Python 3.11+ or uv (run setup-wizard: 'uv python install').")
    exit 1
}

$allArgs = @() + $runnerArgs + @($convertPy, '--doctor') + $DoctorArgs
& $workingRunner @allArgs
exit $LASTEXITCODE
