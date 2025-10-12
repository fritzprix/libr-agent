# Usage: .\dump_error.ps1 [LINES]
# Default: 20 lines
param(
    [int]$Lines = 20
)

function Get-ErrorContext {
    param(
        [string]$LogFile,
        [string]$OutputFile,
        [string]$Pattern = "[ERROR]",
        [int]$Context = 5
    )
    
    if (-not (Test-Path $LogFile)) {
        Write-Host "❌ 로그 파일을 찾을 수 없습니다: $LogFile" -ForegroundColor Red
        "" | Out-File -FilePath $OutputFile -Encoding UTF8
        return
    }
    
    try {
        $lines = Get-Content $LogFile -Encoding UTF8

        # Collect numeric match indices
        $matchIndices = New-Object System.Collections.Generic.List[int]
        for ($i = 0; $i -lt $lines.Count; $i++) {
            if ($lines[$i] -like "*$Pattern*") {
                [void]$matchIndices.Add([int]$i)
            }
        }

        if ($matchIndices.Count -eq 0) {
            # No matches found, create empty UTF8 file
            "" | Out-File -FilePath $OutputFile -Encoding UTF8
            return
        }

        # Build ranges as ordered hashtables and merge them
        $ranges = @()
        foreach ($idx in $matchIndices) {
            $start = [int]([Math]::Max(0, $idx - $Context))
            $end = [int]([Math]::Min($lines.Count - 1, $idx + $Context))
            $ranges += @{ Start = $start; End = $end }
        }

        $ranges = $ranges | Sort-Object -Property Start

        $merged = @()
        foreach ($r in $ranges) {
            if ($merged.Count -eq 0) {
                $merged += $r
                continue
            }
            $last = $merged[-1]
            if ($r.Start -le ($last.End + 1)) {
                # extend the last range
                $last.End = [Math]::Max($last.End, $r.End)
                $merged[-1] = $last
            } else {
                $merged += $r
            }
        }

        # Extract lines for merged ranges
        $outLines = New-Object System.Collections.Generic.List[string]
        foreach ($m in $merged) {
            for ($i = $m.Start; $i -le $m.End; $i++) {
                [void]$outLines.Add($lines[$i])
            }
            [void]$outLines.Add("")
        }

        $outLines | Out-File -FilePath $OutputFile -Encoding UTF8

    } catch {
        # Write detailed error to host for debugging, but still create an empty UTF8 file
        Write-Host "❌ 에러 컨텍스트 추출 실패: $($_.Exception.Message)" -ForegroundColor Red
        if ($_.Exception.InnerException) {
            Write-Host "Inner: $($_.Exception.InnerException.Message)" -ForegroundColor Red
        }
        "" | Out-File -FilePath $OutputFile -Encoding UTF8
    }
}

Write-Host "🔍 Extracting error logs..." -ForegroundColor Cyan
Write-Host "📊 Lines to extract: $Lines" -ForegroundColor Blue

# Call dump_log.ps1 script
& ".\dump_log.ps1" -Lines $Lines
$dumpLogExitCode = $LASTEXITCODE

if ($dumpLogExitCode -eq 0) {
    Write-Host "✅ Log extraction completed" -ForegroundColor Green
    Write-Host "🔧 Processing error context..." -ForegroundColor Yellow
    
    # Use the internal PowerShell extractor to avoid external encoding issues
    if (Test-Path "error.txt") {
        Remove-Item "error.txt" -Force -ErrorAction SilentlyContinue
    }
    Write-Host "ℹ️ Using internal PowerShell extractor for error context" -ForegroundColor Cyan
    Get-ErrorContext -LogFile ".\log.txt" -OutputFile "error.txt"
    
    Write-Host "✅ Error context saved to error.txt" -ForegroundColor Green
    
    if (Test-Path "error.txt") {
        $lineCount = (Get-Content "error.txt").Count
        Write-Host "📄 Total lines in error.txt: $lineCount" -ForegroundColor Cyan
    } else {
        Write-Host "📄 Total lines in error.txt: 0" -ForegroundColor Cyan
    }
} else {
    Write-Host "❌ Log extraction failed" -ForegroundColor Red
    exit 1
}