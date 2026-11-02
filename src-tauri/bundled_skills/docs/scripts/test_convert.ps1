[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$skillDir = Split-Path -Parent $scriptDir
$convertPy = Join-Path $scriptDir 'convert.py'

Write-Host "=== Testing docs skill convert.py ==="

# ---------------------------------------------------------
# Acceptance Criterion 2: Forced failure (missing input file)
# Must exit non-zero, print NEXT: line, print no OK line.
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

if ($allText -notmatch 'NEXT:') {
  Write-Error "Test 1 failed: Expected output to contain 'NEXT:' hint line"
  exit 1
}

if ($allText -match '(^|\r?\n)OK($|\r?\n)') {
  Write-Error "Test 1 failed: Output must NOT contain 'OK' on failure"
  exit 1
}
Write-Host "✓ Test 1 passed: Forced failure exited non-zero with NEXT: and no OK."

# ---------------------------------------------------------
# Acceptance Criterion 1: convert.py --doctor succeeds when
# python on PATH fails with exit code -1073741515.
# ---------------------------------------------------------
Write-Host "`n[Test 2] convert.py --doctor with broken python on PATH (-1073741515)..."

$fakeDir = Join-Path ([System.IO.Path]::GetTempPath()) "fake_python_$(Get-Random)"
New-Item -ItemType Directory -Path $fakeDir -Force | Out-Null
$fakePythonCmd = Join-Path $fakeDir "python.cmd"
Set-Content -Path $fakePythonCmd -Value "@exit /b -1073741515"

$origPath = $env:PATH
try {
  # Prepend broken python to PATH
  $env:PATH = "$fakeDir;$origPath"

  # Verify python on PATH now fails with -1073741515
  & python.cmd 2>$null
  $brokenCode = $LASTEXITCODE
  Write-Host "Simulated broken python exit code: $brokenCode"

  # Run convert.py --doctor using a known functional python launcher / runner
  $doctorLauncher = $null
  if (Get-Command uv -ErrorAction SilentlyContinue) {
    $doctorLauncher = 'uv'
  } elseif (Get-Command py -ErrorAction SilentlyContinue) {
    $doctorLauncher = 'py'
  }

  if ($doctorLauncher -eq 'uv') {
    $doctorOutput = & uv run --no-project python $convertPy --doctor 2>&1
  } elseif ($doctorLauncher -eq 'py') {
    $doctorOutput = & py -3 $convertPy --doctor 2>&1
  } else {
    # If no launcher, resolve real python outside fakeDir
    $realPy = (Get-Command -Name python.exe -All | Where-Object { $_.Source -notlike "$fakeDir*" } | Select-Object -First 1).Source
    $doctorOutput = & $realPy $convertPy --doctor 2>&1
  }
  $doctorExit = $LASTEXITCODE

  $doctorText = ($doctorOutput -join "`n").Trim()
  Write-Host "Doctor output:`n$doctorText"
  Write-Host "Doctor exit code: $doctorExit"

  if ($doctorExit -ne 0) {
    Write-Error "Test 2 failed: convert.py --doctor exited with code $doctorExit"
    exit 1
  }

  if ($doctorText -notmatch 'Doctor: OK' -or $doctorText -notmatch 'Venv:') {
    Write-Error "Test 2 failed: Doctor output missing 'Doctor: OK' or 'Venv:'"
    exit 1
  }
  Write-Host "✓ Test 2 passed: convert.py --doctor succeeded with broken PATH python."
} finally {
  $env:PATH = $origPath
  if (Test-Path $fakeDir) {
    Remove-Item -Recurse -Force $fakeDir -ErrorAction SilentlyContinue
  }
}

# ---------------------------------------------------------
# Test 3: End-to-end basic conversion command form
# ---------------------------------------------------------
Write-Host "`n[Test 3] End-to-end conversion command form..."
$testMd = Join-Path ([System.IO.Path]::GetTempPath()) "test_doc_$(Get-Random).md"
$testOut = Join-Path ([System.IO.Path]::GetTempPath()) "test_out_$(Get-Random).pdf"
Set-Content -Path $testMd -Value "# Test Document`n`nSample content for conversion."

try {
  $convOutput = & python $convertPy --in $testMd --to pdf --template report --out $testOut 2>&1
  $convExit = $LASTEXITCODE
  $convText = ($convOutput -join "`n").Trim()

  Write-Host "Conversion exit code: $convExit"
  Write-Host "Conversion output:`n$convText"

  if ($convExit -ne 0) {
    Write-Error "Test 3 failed: Conversion exited with code $convExit"
    exit 1
  }

  if (-not (Test-Path $testOut)) {
    Write-Error "Test 3 failed: Output file was not created: $testOut"
    exit 1
  }

  if ($convText -notmatch 'Output:' -or $convText -notmatch 'Size:' -or $convText -notmatch 'ExitCode: 0' -or $convText -notmatch '(^|\r?\n)OK($|\r?\n)') {
    Write-Error "Test 3 failed: Output contract missing required fields (Output, Size, ExitCode, OK)"
    exit 1
  }
  Write-Host "✓ Test 3 passed: Conversion output contract met."
} finally {
  if (Test-Path $testMd) { Remove-Item $testMd -Force -ErrorAction SilentlyContinue }
  if (Test-Path $testOut) { Remove-Item $testOut -Force -ErrorAction SilentlyContinue }
}

Write-Host "`n=== All docs skill tests passed successfully! ==="
