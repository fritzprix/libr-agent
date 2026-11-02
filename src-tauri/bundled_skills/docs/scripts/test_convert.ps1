[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$skillDir = Split-Path -Parent $scriptDir
$convertPy = Join-Path $scriptDir 'convert.py'
$doctorPs1 = Join-Path $scriptDir 'doctor.ps1'

Write-Host "=== Testing docs skill convert.py ==="

# ---------------------------------------------------------
# Test 1: Forced failure on missing input file (G1.4)
# Must exit non-zero, print ExitCode: 1, print NEXT: line, print no OK line.
# ---------------------------------------------------------
Write-Host "`n[Test 1] Forced failure on missing input file..."
$missingFile = Join-Path ([System.IO.Path]::GetTempPath()) "nonexistent_$(Get-Random).md"
$tempOut = Join-Path ([System.IO.Path]::GetTempPath()) "out_$(Get-Random).pdf"

$output = & python $convertPy --in $missingFile --to pdf --template report --out $tempOut 2>&1
$exitCode = $LASTEXITCODE

Write-Host "Exit code: $exitCode"
$allText = ($output -join "`n").Trim()
Write-Host "Output: $allText"

if ($exitCode -eq 0) {
  Write-Error "Test 1 failed: Expected non-zero exit code on missing input file, got $exitCode"
  exit 1
}

if ($allText -notmatch 'ExitCode:\s*1') {
  Write-Error "Test 1 failed: Expected output to contain 'ExitCode: 1'"
  exit 1
}

if ($allText -notmatch 'NEXT:') {
  Write-Error "Test 1 failed: Expected output to contain 'NEXT:' hint line"
  exit 1
}

if ($allText -match '(^|\r?\n)OK($|\r?\n)') {
  Write-Error "Test 1 failed: Output must NOT contain 'OK' on failure"
  exit 1
}
Write-Host "Test 1 passed: Forced failure exited non-zero with ExitCode: 1, NEXT: and no OK."

# ---------------------------------------------------------
# Test 2: Documented doctor command with broken python on PATH (-1073741515) (G1.2)
# Runs EXACTLY the documented doctor command with fake python.cmd first on PATH.
# ---------------------------------------------------------
Write-Host "`n[Test 2] Documented doctor command with broken python on PATH (-1073741515)..."

$fakeDir = Join-Path ([System.IO.Path]::GetTempPath()) "fake_python_$(Get-Random)"
New-Item -ItemType Directory -Path $fakeDir -Force | Out-Null
$fakePythonCmd = Join-Path $fakeDir "python.cmd"
Set-Content -Path $fakePythonCmd -Value "@exit /b -1073741515"

$origPath = $env:PATH
$doctorText = ""
try {
  # Prepend broken python to PATH
  $env:PATH = "$fakeDir;$origPath"

  # Verify python on PATH now fails with -1073741515
  & python.cmd 2>$null
  $brokenCode = $LASTEXITCODE
  Write-Host "Simulated broken python exit code: $brokenCode"

  # Run exactly the documented doctor command
  $doctorOutput = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $doctorPs1 2>&1
  $doctorExit = $LASTEXITCODE

  $doctorText = ($doctorOutput -join "`n").Trim()
  Write-Host "Doctor output:`n$doctorText"
  Write-Host "Doctor exit code: $doctorExit"

  if ($doctorExit -ne 0) {
    Write-Error "Test 2 failed: Documented doctor command exited with code $doctorExit"
    exit 1
  }

  if ($doctorText -notmatch 'Doctor:\s*OK' -or $doctorText -notmatch 'Venv:\s*(.+)' -or $doctorText -notmatch 'VenvPython:\s*(.+)') {
    Write-Error "Test 2 failed: Doctor output missing 'Doctor: OK', 'Venv:', or 'VenvPython:'"
    exit 1
  }

  if ($doctorText -notmatch 'ExitCode:\s*0') {
    Write-Error "Test 2 failed: Doctor output missing 'ExitCode: 0'"
    exit 1
  }
  Write-Host "Test 2 passed: Documented doctor command succeeded with broken PATH python."
} finally {
  $env:PATH = $origPath
  if (Test-Path $fakeDir) {
    Remove-Item -Recurse -Force $fakeDir -ErrorAction SilentlyContinue
  }
}

# ---------------------------------------------------------
# Test 3: Skeleton conversion command form with doctor VenvPython (G1.1 & G2.1)
# Must run exact documented command with leading '&', exit non-zero,
# print ExitCode: 1, print NEXT: naming #2498, and print no OK.
# ---------------------------------------------------------
Write-Host "`n[Test 3] Skeleton conversion command form with doctor VenvPython (pending engine)..."
$testMd = Join-Path ([System.IO.Path]::GetTempPath()) "test_doc_$(Get-Random).md"
$testOut = Join-Path ([System.IO.Path]::GetTempPath()) "test_out_$(Get-Random).pdf"
$testScript = Join-Path ([System.IO.Path]::GetTempPath()) "test_doc_cmd_$(Get-Random).ps1"
Set-Content -Path $testMd -Value "# Test Document`n`nSample content for conversion."

$venvPython = $null
foreach ($line in ($doctorText -split "`r?`n")) {
  if ($line -match '^VenvPython:\s*(.+)$') {
    $venvPython = $Matches[1].Trim()
    break
  }
}
if (-not $venvPython -or -not (Test-Path $venvPython)) {
  $venvPython = 'python'
}
Write-Host "Using VenvPython: $venvPython"

# S3.1: Read the documented command directly from SKILL.md and substitute placeholders
$skillMdPath = Join-Path $skillDir "SKILL.md"
$skillMdContent = Get-Content -Raw $skillMdPath
if ($skillMdContent -match '(?m)^```powershell\r?\n(&\s*"<venv-python-from-doctor>".+?)\r?\n```') {
  $documentedTemplate = $Matches[1].Trim()
} else {
  Write-Error 'Test 3 failed: Could not find documented conversion command starting with call operator in SKILL.md'
  exit 1
}

$documentedCommand = $documentedTemplate `
  -replace '<venv-python-from-doctor>', $venvPython `
  -replace '<skill-base-dir>', $skillDir `
  -replace '<abs\.md>', $testMd `
  -replace '<abs\.pdf>', $testOut

Write-Host "Documented command: $documentedCommand"
Set-Content -Path $testScript -Value "$documentedCommand`r`nexit `$LASTEXITCODE"

try {
  $convOutput = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $testScript 2>&1
  $convExit = $LASTEXITCODE
  $convText = ($convOutput -join "`n").Trim()

  Write-Host "Conversion exit code: $convExit"
  Write-Host "Conversion output:`n$convText"

  if ($convExit -eq 0) {
    Write-Error "Test 3 failed: Skeleton conversion must NOT exit 0 before format engine is implemented"
    exit 1
  }

  if ($convText -notmatch 'ExitCode:\s*1') {
    Write-Error "Test 3 failed: Expected output to contain 'ExitCode: 1'"
    exit 1
  }

  if ($convText -notmatch 'NEXT:.*#2498') {
    Write-Error "Test 3 failed: Expected NEXT: line to reference sub-issue #2498"
    exit 1
  }

  if ($convText -match '(^|\r?\n)OK($|\r?\n)') {
    Write-Error "Test 3 failed: Output must NOT contain 'OK' for unimplemented skeleton conversion"
    exit 1
  }

  if (Test-Path $testOut) {
    Write-Error "Test 3 failed: Output file must NOT be created for unimplemented conversion: $testOut"
    exit 1
  }

  Write-Host "Test 3 passed: Documented command '& `"<venv>`" `"<convert>`" ...' executed successfully via PowerShell script and correctly exited non-zero with ExitCode: 1, NEXT: #2498, and no OK."
} finally {
  if (Test-Path $testScript) { Remove-Item $testScript -Force -ErrorAction SilentlyContinue }
  if (Test-Path $testMd) { Remove-Item $testMd -Force -ErrorAction SilentlyContinue }
  if (Test-Path $testOut) { Remove-Item $testOut -Force -ErrorAction SilentlyContinue }
}

Write-Host "`n=== All docs skill tests passed successfully! ==="
