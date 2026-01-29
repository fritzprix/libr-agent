# Usage: .\dump_error.ps1 [LINES]
# Default: 20 lines
param(
    [int]$Lines = 20
)

$ErrorActionPreference = "Stop"

Write-Host "🔍 Extracting error logs..." -ForegroundColor Cyan
Write-Host "📊 Lines to extract: $Lines" -ForegroundColor Cyan

# Run dump_log.ps1
# Note: dump_log.ps1 handles its own output messages
& .\dump_log.ps1 -Lines $Lines

if ($LASTEXITCODE -eq 0) {
    Write-Host "✅ Log extraction completed" -ForegroundColor Green
    Write-Host "🔧 Processing error context..." -ForegroundColor Cyan

    # Define paths
    $ScriptDir = $PSScriptRoot
    $PyScript = Join-Path $ScriptDir "scripts\extract_context.py"
    $LogFile = "log.txt"
    $ErrorFile = "error.txt"

    # Check for python
    if (Get-Command "python" -ErrorAction SilentlyContinue) {
        $PythonExe = "python"
    }
    elseif (Get-Command "python3" -ErrorAction SilentlyContinue) {
        $PythonExe = "python3"
    }
    else {
        Write-Error "python not found in PATH"
        exit 1
    }

    try {
        # Run extraction and capture output to file
        # Using direct execution with output file argument to avoid pipe encoding issues
        & $PythonExe $PyScript $LogFile --pattern "[ERROR]" --context 5 --output $ErrorFile
        
        if ($LASTEXITCODE -ne 0) {
            throw "Python script failed with exit code $LASTEXITCODE"
        }

        Write-Host "✅ Error context saved to $ErrorFile" -ForegroundColor Green
        
        # Count lines in error.txt
        if (Test-Path $ErrorFile) {
            $Count = (Get-Content $ErrorFile | Measure-Object).Count
            Write-Host "📄 Total lines in $ErrorFile : $Count" -ForegroundColor White
        }
    }
    catch {
        Write-Host "❌ Error processing failed: $_" -ForegroundColor Red
        exit 1
    }
}
else {
    Write-Host "❌ Log extraction failed" -ForegroundColor Red
    exit 1
}
