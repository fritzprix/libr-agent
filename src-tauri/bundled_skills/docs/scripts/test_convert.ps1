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
# Test 3: Markdown to PDF with doctor VenvPython (G2.1)
# Must run the exact documented command with leading '&', exit 0,
# print ExitCode: 0 and an OK line, and create a real PDF.
# ---------------------------------------------------------
Write-Host "`n[Test 3] Markdown to PDF with doctor VenvPython..."

$tempRoot = [System.IO.Path]::GetTempPath()
$testMd = Join-Path $tempRoot "test_doc_$(Get-Random).md"
$testOut = Join-Path $tempRoot "test_out_$(Get-Random).pdf"
$testScript = Join-Path $tempRoot "test_doc_cmd_$(Get-Random).ps1"

# Korean wide table: 7 columns with long cells. This is the acceptance case.
$koreanMd = @'
# 분기 운영 점검 보고서

본 문서는 분기 운영 결과를 정리한 보고서입니다. ✅ 전 항목 정상입니다.

## 1. 상세 지표

| 구분 | 세부 항목 | 계획 | 실적 | 달성률 | 담당 부서 | 담당자 |
|---|---|---|---|---|---|---|
| 영업 | 국내 기업 고객 신규 계약 체결 건수 | 120건 | 141건 | 118% | 영업기획팀 | 김민수 |
| 재무 | 인건비 및 복리후생 예산 집행률 | 45억 | 44억 | 98% | 경영지원팀 | 박준호 |
| 품질 | 서비스 운영 중 발생 결함률 관리 | 0.5% | 0.3% | 60% | 품질보증팀 | 정하늘 |

## 2. 결론

⭐ 전반적으로 목표를 달성했습니다.
'@
Set-Content -Path $testMd -Value $koreanMd -Encoding UTF8

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

# Read the documented command directly from SKILL.md and substitute placeholders
$skillMdPath = Join-Path $skillDir "SKILL.md"
$skillMdContent = Get-Content -Raw $skillMdPath
if ($skillMdContent -match '(?ms)^```powershell\r?\n(&\s*"<venv-python-from-doctor>".*?)\r?\n```') {
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

  if ($convExit -ne 0) {
    Write-Error "Test 3 failed: PDF conversion must exit 0, got $convExit"
    exit 1
  }

  if ($convText -notmatch 'ExitCode:\s*0') {
    Write-Error "Test 3 failed: Expected output to contain 'ExitCode: 0'"
    exit 1
  }

  if ($convText -notmatch '(^|\r?\n)OK(\s|$)') {
    Write-Error "Test 3 failed: Expected an OK line on success"
    exit 1
  }

  if (-not (Test-Path $testOut)) {
    Write-Error "Test 3 failed: Output PDF was not created: $testOut"
    exit 1
  }

  $pdfSize = (Get-Item $testOut).Length
  if ($pdfSize -le 0) {
    Write-Error "Test 3 failed: Output PDF is 0 bytes"
    exit 1
  }
  Write-Host "Test 3 passed: Korean wide-table Markdown rendered to a $pdfSize byte PDF with ExitCode: 0 and OK."

  # ---------------------------------------------------------
  # Test 4: check.py rejects a 0-byte PDF
  # ---------------------------------------------------------
  Write-Host "`n[Test 4] check.py rejects a 0-byte PDF..."
  $checkPy = Join-Path $scriptDir 'check.py'
  $emptyOut = Join-Path $tempRoot "empty_$(Get-Random).pdf"
  Set-Content -Path $emptyOut -Value '' -NoNewline

  $checkOutput = & $venvPython $checkPy --pdf $emptyOut 2>&1
  $checkExit = $LASTEXITCODE
  Write-Host "check.py exit code: $checkExit"
  Write-Host "check.py output:`n$(($checkOutput -join "`n").Trim())"

  if ($checkExit -eq 0) {
    Write-Error "Test 4 failed: check.py must reject a 0-byte PDF"
    exit 1
  }
  if ((($checkOutput -join "`n") -match '(^|\r?\n)OK(\s|$)')) {
    Write-Error "Test 4 failed: check.py must not print OK for a 0-byte PDF"
    exit 1
  }
  Remove-Item $emptyOut -Force -ErrorAction SilentlyContinue
  Write-Host "Test 4 passed: check.py rejected the 0-byte PDF."

  # ---------------------------------------------------------
  # Test 5: symbols are decided by the font cmap, not by Unicode range
  # ---------------------------------------------------------
  Write-Host "`n[Test 5] Symbols the font can draw are kept; others become markers..."
  $symMd = Join-Path $tempRoot "symbols_$(Get-Random).md"
  $symOut = Join-Path $tempRoot "symbols_$(Get-Random).pdf"
  $symSource = @'
# 기호

평점 ★★★☆☆ 그리고 ♥ ☎ ✓

결과 ✅ 이고 위험 ⭕ 는 없습니다.
'@
  Set-Content -Path $symMd -Value $symSource -Encoding UTF8
  $symOutput = & $venvPython $convertPy --in $symMd --to pdf --template minutes --out $symOut 2>&1
  $symExit = $LASTEXITCODE
  if ($symExit -ne 0) {
    Write-Error "Test 5 failed: symbol conversion exited with $symExit`n$(($symOutput -join "`n").Trim())"
    exit 1
  }
  # Read the text back: pdfplumber reports the drawn characters.
  $symDump = & $venvPython -c "import pdfplumber,sys; print(''.join((p.extract_text() or '') for p in pdfplumber.open(sys.argv[1]).pages))" $symOut
  $symText = $symDump -join "`n"
  foreach ($kept in @([char]0x2605, [char]0x2665, [char]0x260E, [char]0x2713)) {
    if (-not $symText.Contains($kept)) {
      Write-Error "Test 5 failed: the font covers $kept but the rendered PDF dropped it."
      exit 1
    }
  }
  foreach ($marker in @('[OK]', '[o]')) {
    if (-not $symText.Contains($marker)) {
      Write-Error "Test 5 failed: expected fallback marker $marker in the rendered PDF."
      exit 1
    }
  }
  Remove-Item $symMd, $symOut -Force -ErrorAction SilentlyContinue
  Write-Host "Test 5 passed: drawable symbols kept, undrawable ones replaced by markers."

  # ---------------------------------------------------------
  # Test 6: empty Markdown fails instead of producing an empty document
  # ---------------------------------------------------------
  Write-Host "`n[Test 6] Empty Markdown fails with a NEXT hint..."
  $blankMd = Join-Path $tempRoot "blank_$(Get-Random).md"
  $blankOut = Join-Path $tempRoot "blank_$(Get-Random).pdf"
  Set-Content -Path $blankMd -Value "   `n`n" -NoNewline
  $blankOutput = & $venvPython $convertPy --in $blankMd --to pdf --template report --out $blankOut 2>&1
  $blankExit = $LASTEXITCODE
  $blankText = $blankOutput -join "`n"
  if ($blankExit -eq 0) {
    Write-Error "Test 6 failed: empty Markdown must not succeed."
    exit 1
  }
  if ($blankText -notmatch 'NEXT:') {
    Write-Error "Test 6 failed: empty Markdown must print a NEXT: hint."
    exit 1
  }
  if ($blankText -match '(^|\r?\n)OK(\s|$)') {
    Write-Error "Test 6 failed: empty Markdown must not print OK."
    exit 1
  }
  Remove-Item $blankMd, $blankOut -Force -ErrorAction SilentlyContinue
  Write-Host "Test 6 passed: empty Markdown failed with a NEXT hint and no OK line."
  # ---------------------------------------------------------
  # Test 7: each ordered list gets its own Word numbering
  # ---------------------------------------------------------
  Write-Host "`n[Test 7] Separate ordered lists restart their numbering..."
  $numMd = Join-Path $tempRoot "numbering_$(Get-Random).md"
  $numOut = Join-Path $tempRoot "numbering_$(Get-Random).docx"
  $numSource = @'
# 번호

## 첫 목록

1. 하나
2. 둘

## 둘째 목록

5. 다섯
6. 여섯

## 셋째 목록

1. 다시 하나
'@
  Set-Content -Path $numMd -Value $numSource -Encoding UTF8
  $numOutput = & $venvPython $convertPy --in $numMd --to docx --template report --out $numOut 2>&1
  $numExit = $LASTEXITCODE
  if ($numExit -ne 0) {
    Write-Error "Test 7 failed: DOCX numbering conversion exited with $numExit`n$(($numOutput -join "`n").Trim())"
    exit 1
  }
  # Each separate Markdown list must own a distinct numId, and the list that
  # starts at 5 must carry that start value.
  $numProbe = & $venvPython -c @"
import sys
from docx import Document
from docx.oxml.ns import qn
doc = Document(sys.argv[1])
numbering = doc.part.numbering_part.element
seen = []
for para in doc.paragraphs:
    if para.style.name != 'List Number':
        continue
    num_pr = para._p.pPr.find(qn('w:numPr'))
    if num_pr is None:
        print('MISSING_NUMPR'); sys.exit(0)
    seen.append(num_pr.find(qn('w:numId')).get(qn('w:val')))
starts = {}
for num in numbering.findall(qn('w:num')):
    override = num.find(qn('w:lvlOverride'))
    if override is not None:
        starts[num.get(qn('w:numId'))] = override.find(qn('w:startOverride')).get(qn('w:val'))
distinct = []
for item in seen:
    if item not in distinct:
        distinct.append(item)
print('PARAS=' + ','.join(seen))
print('DISTINCT=' + ','.join(distinct))
print('STARTS=' + ','.join(str(starts.get(item, '?')) for item in distinct))
"@ $numOut
  $numText = $numProbe -join "`n"
  if ($numText -match 'MISSING_NUMPR') {
    Write-Error "Test 7 failed: an ordered paragraph has no w:numPr."
    exit 1
  }
  $distinctLine = ($numProbe | Where-Object { $_ -like 'DISTINCT=*' }) -replace '^DISTINCT=', ''
  $distinctIds = $distinctLine -split ','
  if ($distinctIds.Count -ne 3) {
    Write-Error "Test 7 failed: expected 3 independent lists, got $($distinctIds.Count) ($distinctLine)."
    exit 1
  }
  $startsLine = ($numProbe | Where-Object { $_ -like 'STARTS=*' }) -replace '^STARTS=', ''
  $startValues = $startsLine -split ','
  if ($startValues[0] -ne '1' -or $startValues[1] -ne '5' -or $startValues[2] -ne '1') {
    Write-Error "Test 7 failed: expected list starts 1,5,1 but got $startsLine."
    exit 1
  }
  Remove-Item $numMd, $numOut -Force -ErrorAction SilentlyContinue
  Write-Host "Test 7 passed: 3 separate lists with independent counters and a 5 start."

  # ---------------------------------------------------------
  # Test 8: a long bullet list splits across slides
  # ---------------------------------------------------------
  Write-Host "`n[Test 8] Long bullet lists split across slides..."
  $manyMd = Join-Path $tempRoot "many_$(Get-Random).md"
  $manyOut = Join-Path $tempRoot "many_$(Get-Random).pptx"
  $manyLines = @('# 다량 항목', '', '## 많은 목록', '')
  foreach ($n in 1..12) { $manyLines += "- 항목 $n" }
  Set-Content -Path $manyMd -Value ($manyLines -join "`n") -Encoding UTF8
  $manyOutput = & $venvPython $convertPy --in $manyMd --to pptx --template report --out $manyOut 2>&1
  $manyExit = $LASTEXITCODE
  if ($manyExit -ne 0) {
    Write-Error "Test 8 failed: PPTX conversion exited with $manyExit`n$(($manyOutput -join "`n").Trim())"
    exit 1
  }
  $manyProbe = & $venvPython -c "import sys; from pptx import Presentation; p=Presentation(sys.argv[1]); print('SLIDES='+str(len(p.slides))); print('|'.join(sh.text_frame.text.replace(chr(10),' ') for s in p.slides for sh in s.shapes if sh.has_text_frame))" $manyOut
  $manyText = $manyProbe -join "`n"
  if ($manyText -notmatch 'SLIDES=3') {
    Write-Error "Test 8 failed: expected 3 slides (cover, list, continued), got $($manyText -split "`n" | Select-Object -First 1)."
    exit 1
  }
  if ($manyText -notmatch '\(continued\)') {
    Write-Error "Test 8 failed: the overflow slide title must end with (continued)."
    exit 1
  }
  Remove-Item $manyMd, $manyOut -Force -ErrorAction SilentlyContinue
  Write-Host "Test 8 passed: 12 bullets split over 2 content slides with a (continued) title."

} finally {
  if (Test-Path $testScript) { Remove-Item $testScript -Force -ErrorAction SilentlyContinue }
  if (Test-Path $testMd) { Remove-Item $testMd -Force -ErrorAction SilentlyContinue }
}

Write-Host "`n=== All docs skill tests passed successfully! ==="
