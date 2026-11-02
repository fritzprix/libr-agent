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

  # ---------------------------------------------------------
  # Test 9: identical list items are valid and must convert (G1.2)
  # Repeated items such as '- TBD' are ordinary Markdown, so the checker must
  # not treat them as a duplicated-paragraph layout defect.
  # ---------------------------------------------------------
  Write-Host "`n[Test 9] Identical list items convert with OK..."
  $dupMd = Join-Path $tempRoot "dup_$(Get-Random).md"
  $dupOut = Join-Path $tempRoot "dup_$(Get-Random).docx"
  $dupSource = @'
# 확인

## 점검

- TBD
- TBD
- TBD
'@
  Set-Content -Path $dupMd -Value $dupSource -Encoding UTF8
  $dupOutput = & $venvPython $convertPy --in $dupMd --to docx --template report --out $dupOut 2>&1
  $dupExit = $LASTEXITCODE
  if ($dupExit -ne 0) {
    Write-Error "Test 9 failed: identical list items must convert, exit $dupExit`n$(($dupOutput -join "`n").Trim())"
    exit 1
  }
  $dupCheck = & $venvPython $checkPy --docx $dupOut 2>&1
  $dupCheckExit = $LASTEXITCODE
  $dupCheckText = ($dupCheck -join "`n").Trim()
  if ($dupCheckExit -ne 0 -or $dupCheckText -notmatch 'OK') {
    Write-Error "Test 9 failed: check.py must accept identical list items`n$dupCheckText"
    exit 1
  }
  Remove-Item $dupMd, $dupOut -Force -ErrorAction SilentlyContinue
  Write-Host "Test 9 passed: three identical '- TBD' items convert with OK."

  # ---------------------------------------------------------
  # Test 10: a Hangul run without w:eastAsia is rejected (G1.1a)
  # Word asks the Latin font for Hangul glyphs when a run names no east-Asian
  # face, which is the missing-glyph cause check_docx must catch.
  # ---------------------------------------------------------
  Write-Host "`n[Test 10] check.py rejects a Hangul run with no w:eastAsia font..."
  $noEastOut = Join-Path $tempRoot "noeast_$(Get-Random).docx"
  $noEastProbe = & $venvPython -c @'
import sys

import docx
from docx.shared import Pt

document = docx.Document()
paragraph = document.add_paragraph()
run = paragraph.add_run("\ud55c\uae00 \ud14d\uc2a4\ud2b8\uc785\ub2c8\ub2e4")
run.font.name = "Malgun Gothic"
run.font.size = Pt(12)
document.save(sys.argv[1])
print("WROTE")
'@ $noEastOut
  if (($noEastProbe -join "`n") -notmatch 'WROTE') {
    Write-Error "Test 10 failed: could not build the negative DOCX fixture`n$(($noEastProbe -join "`n").Trim())"
    exit 1
  }
  $noEastCheck = & $venvPython $checkPy --docx $noEastOut 2>&1
  $noEastExit = $LASTEXITCODE
  $noEastText = ($noEastCheck -join "`n").Trim()
  if ($noEastExit -eq 0) {
    Write-Error "Test 10 failed: a Hangul run without w:eastAsia must not pass`n$noEastText"
    exit 1
  }
  if ($noEastText -notmatch 'w:eastAsia') {
    Write-Error "Test 10 failed: the failure must name w:eastAsia`n$noEastText"
    exit 1
  }
  Remove-Item $noEastOut -Force -ErrorAction SilentlyContinue
  Write-Host "Test 10 passed: missing east-Asian font reported as a failure."

  # ---------------------------------------------------------
  # Test 11: content that runs past the slide bottom is rejected (G1.1b)
  # PowerPoint grows a table row or a text box that needs more room than it
  # declares, so a declared shape can fit while the drawn text does not.
  # ---------------------------------------------------------
  Write-Host "`n[Test 11] check.py rejects content past the slide bottom..."
  $tallTableOut = Join-Path $tempRoot "talltable_$(Get-Random).pptx"
  $tallTextOut = Join-Path $tempRoot "talltext_$(Get-Random).pptx"
  $tallProbe = & $venvPython -c @'
import sys

from pptx import Presentation
from pptx.util import Inches, Pt

table_path, text_path = sys.argv[1], sys.argv[2]

presentation = Presentation()
presentation.slide_width = Inches(13.333)
presentation.slide_height = Inches(7.5)
slide = presentation.slides.add_slide(presentation.slide_layouts[5])
slide.shapes.title.text = "Tall table"
table = slide.shapes.add_table(
    21, 3, Inches(0.8), Inches(1.4), Inches(11.5), Inches(4.0)
).table
for row in range(21):
    for column in range(3):
        table.cell(row, column).text = "cell %d-%d" % (row, column)
presentation.save(table_path)

deck = Presentation()
deck.slide_width = Inches(13.333)
deck.slide_height = Inches(7.5)
body = deck.slides.add_slide(deck.slide_layouts[6])
box = body.shapes.add_textbox(Inches(0.8), Inches(1.2), Inches(11.5), Inches(1.0))
frame = box.text_frame
for line in range(40):
    paragraph = frame.paragraphs[0] if line == 0 else frame.add_paragraph()
    paragraph.text = "code line %d" % line
    paragraph.font.size = Pt(14)
deck.save(text_path)

print("WROTE")
'@ $tallTableOut $tallTextOut
  if (($tallProbe -join "`n") -notmatch 'WROTE') {
    Write-Error "Test 11 failed: could not build the overflow fixtures`n$(($tallProbe -join "`n").Trim())"
    exit 1
  }
  foreach ($tallCase in @(
    @{ Path = $tallTableOut; Label = 'table' },
    @{ Path = $tallTextOut; Label = 'text box' }
  )) {
    $tallCheck = & $venvPython $checkPy --pptx $tallCase.Path 2>&1
    $tallExit = $LASTEXITCODE
    $tallCheckText = ($tallCheck -join "`n").Trim()
    if ($tallExit -eq 0) {
      Write-Error "Test 11 failed: an oversized $($tallCase.Label) must not pass`n$tallCheckText"
      exit 1
    }
    if ($tallCheckText -notmatch 'past the slide bottom') {
      Write-Error "Test 11 failed: the failure must name the slide bottom`n$tallCheckText"
      exit 1
    }
    if ($tallCheckText -notmatch [regex]::Escape($tallCase.Label)) {
      Write-Error "Test 11 failed: the failure must name the $($tallCase.Label)`n$tallCheckText"
      exit 1
    }
  }
  Remove-Item $tallTableOut, $tallTextOut -Force -ErrorAction SilentlyContinue
  Write-Host "Test 11 passed: both an oversized table and an oversized text box are rejected."

  # ---------------------------------------------------------
  # Test 12: a 20-row table and a 40-line code block stay inside the slides (G1.3)
  # Both split by vertical budget instead of block count, and the table repeats
  # its header row on the continuation slide.
  # ---------------------------------------------------------
  Write-Host "`n[Test 12] A tall table and a long code block split across slides..."
  $bigDeckMd = Join-Path $tempRoot "bigdeck_$(Get-Random).md"
  $bigDeckOut = Join-Path $tempRoot "bigdeck_$(Get-Random).pptx"
  $bigDeckLines = @('# Big Deck', '', '## Table', '', '| Item | Owner | Status |', '| --- | --- | --- |')
  foreach ($n in 1..20) { $bigDeckLines += "| Item $n | Owner $n | Status $n |" }
  $bigDeckLines += @('', '## Code', '', '```python')
  foreach ($n in 1..40) { $bigDeckLines += "value_$n = compute($n)" }
  $bigDeckLines += '```'
  Set-Content -Path $bigDeckMd -Value ($bigDeckLines -join "`n") -Encoding UTF8
  $bigDeckOutput = & $venvPython $convertPy --in $bigDeckMd --to pptx --template report --out $bigDeckOut 2>&1
  $bigDeckExit = $LASTEXITCODE
  if ($bigDeckExit -ne 0) {
    Write-Error "Test 12 failed: oversized deck conversion exited with $bigDeckExit`n$(($bigDeckOutput -join "`n").Trim())"
    exit 1
  }
  $bigDeckCheck = & $venvPython $checkPy --pptx $bigDeckOut 2>&1
  $bigDeckCheckExit = $LASTEXITCODE
  $bigDeckCheckText = ($bigDeckCheck -join "`n").Trim()
  if ($bigDeckCheckExit -ne 0) {
    Write-Error "Test 12 failed: check.py rejected the oversized deck`n$bigDeckCheckText"
    exit 1
  }
  $bigDeckProbe = & $venvPython -c @'
import sys

from pptx import Presentation

presentation = Presentation(sys.argv[1])
slides = list(presentation.slides)
table_slides = 0
continued = 0
for slide in slides:
    texts = [
        shape.text_frame.text.strip()
        for shape in slide.shapes
        if shape.has_text_frame
    ]
    if any(getattr(shape, "has_table", False) for shape in slide.shapes):
        table_slides += 1
    if any(text.endswith("(continued)") for text in texts):
        continued += 1
print("SLIDES=%d" % len(slides))
print("TABLESLIDES=%d" % table_slides)
print("CONTINUED=%d" % continued)
'@ $bigDeckOut
  $bigDeckText = ($bigDeckProbe -join "`n").Trim()
  if ($bigDeckText -notmatch 'TABLESLIDES=(\d+)') {
    Write-Error "Test 12 failed: could not read the table slide count`n$bigDeckText"
    exit 1
  }
  $tableSlides = [int]$Matches[1]
  if ($tableSlides -lt 2) {
    Write-Error "Test 12 failed: the 20-row table must split over 2 or more slides`n$bigDeckText"
    exit 1
  }
  if ($bigDeckText -notmatch 'CONTINUED=(\d+)') {
    Write-Error "Test 12 failed: could not read the continuation slide count`n$bigDeckText"
    exit 1
  }
  if ([int]$Matches[1] -lt 2) {
    Write-Error "Test 12 failed: both the table and the code block need a (continued) slide`n$bigDeckText"
    exit 1
  }
  $bigDeckRows = & $venvPython -c @'
import sys

from pptx import Presentation

presentation = Presentation(sys.argv[1])
print(",".join(
    str(len(shape.table.rows))
    for slide in presentation.slides
    for shape in slide.shapes
    if getattr(shape, "has_table", False)
))
'@ $bigDeckOut
  $rowCounts = (($bigDeckRows -join "").Trim()) -split ','
  if ($rowCounts.Count -ne $tableSlides) {
    Write-Error "Test 12 failed: expected $tableSlides table slides, saw $($rowCounts -join ',')."
    exit 1
  }
  foreach ($count in $rowCounts) {
    if ([int]$count -gt 21) {
      Write-Error "Test 12 failed: a table slide still holds $count rows, so it did not split."
      exit 1
    }
  }
  Remove-Item $bigDeckMd, $bigDeckOut -Force -ErrorAction SilentlyContinue
  Write-Host "Test 12 passed: 20-row table and 40-line code split by vertical budget."

  # ---------------------------------------------------------
  # Test 13: report and proposal open with an agenda, and every slide after the
  # cover carries a footer with the deck title beside the number; minutes get
  # neither (G1.4)
  # ---------------------------------------------------------
  Write-Host "`n[Test 13] Agenda slide and footer text follow the template..."
  $deckMd = Join-Path $tempRoot "agenda_$(Get-Random).md"
  $deckSource = @'
# 분기 보고

## 개요

본문입니다.

## 결과

본문입니다.
'@
  Set-Content -Path $deckMd -Value $deckSource -Encoding UTF8
  foreach ($deckCase in @(
    @{ Template = 'report'; Agenda = 1; Footers = 3 },
    @{ Template = 'minutes'; Agenda = 0; Footers = 2 }
  )) {
    $deckName = $deckCase.Template
    $deckOut = Join-Path $tempRoot "agenda_${deckName}_$(Get-Random).pptx"
    $deckOutput = & $venvPython $convertPy --in $deckMd --to pptx --template $deckName --out $deckOut 2>&1
    $deckExit = $LASTEXITCODE
    if ($deckExit -ne 0) {
      Write-Error "Test 13 failed: $deckName conversion exited with $deckExit`n$(($deckOutput -join "`n").Trim())"
      exit 1
    }
    $deckProbe = & $venvPython -c @'
import sys

from pptx import Presentation
from pptx.util import Inches

presentation = Presentation(sys.argv[1])
slides = list(presentation.slides)
agenda = 0
footers = 0
for slide in slides:
    texts = [
        shape.text_frame.text.strip()
        for shape in slide.shapes
        if shape.has_text_frame and shape.top is not None
    ]
    if any(text.startswith("Agenda") for text in texts):
        agenda += 1
    low = [
        shape.text_frame.text.strip()
        for shape in slide.shapes
        if shape.has_text_frame and shape.top is not None and shape.top >= Inches(6.9)
    ]
    named = any(text and not text.isdigit() for text in low)
    numbered = any(text.isdigit() for text in low)
    if named and numbered:
        footers += 1
print("AGENDA=%d" % agenda)
print("FOOTERS=%d" % footers)
'@ $deckOut
    $deckText = ($deckProbe -join "`n").Trim()
    $deckLines = $deckText -split "`r?`n"
    if ($deckLines -notcontains "AGENDA=$($deckCase.Agenda)") {
      Write-Error "Test 13 failed: $deckName needs AGENDA=$($deckCase.Agenda)`n$deckText"
      exit 1
    }
    if ($deckLines -notcontains "FOOTERS=$($deckCase.Footers)") {
      Write-Error "Test 13 failed: $deckName needs FOOTERS=$($deckCase.Footers)`n$deckText"
      exit 1
    }
    Remove-Item $deckOut -Force -ErrorAction SilentlyContinue
    Write-Host "Test 13 ($deckName) passed: AGENDA=$($deckCase.Agenda), FOOTERS=$($deckCase.Footers)."
  }
  Remove-Item $deckMd -Force -ErrorAction SilentlyContinue

} finally {
  if (Test-Path $testScript) { Remove-Item $testScript -Force -ErrorAction SilentlyContinue }
  if (Test-Path $testMd) { Remove-Item $testMd -Force -ErrorAction SilentlyContinue }
}

Write-Host "`n=== All docs skill tests passed successfully! ==="
