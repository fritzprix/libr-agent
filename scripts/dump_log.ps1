# Usage: .\dump_log.ps1 [LINES] [SRC_LOG] [OUT_FILE]
param(
    [int]$Lines = 10,
    [string]$SrcLog = "",
    [string]$OutFile = ".\log.txt"
)

# Get default log path based on Windows Tauri log plugin implementation
# Using LibrAgent project name for consistency across all paths
# Note: Windows uses LocalAppData, not AppData\Roaming
function Get-DefaultLogPath {
    $localAppData = [Environment]::GetFolderPath('LocalApplicationData')
    return Join-Path $localAppData "com.fritzprix.libragent\logs\libragent.log"
}

# Set source log path
if ([string]::IsNullOrEmpty($SrcLog)) {
    $SrcLog = Get-DefaultLogPath
}

Write-Host "📊 Lines to extract: $Lines" -ForegroundColor Cyan
Write-Host "📂 Source log: $SrcLog" -ForegroundColor Gray
Write-Host "📄 Output file: $OutFile" -ForegroundColor Gray

# Check if log file exists
if (-not (Test-Path $SrcLog)) {
    Write-Host "⚠️  로그 파일이 존재하지 않습니다: $SrcLog" -ForegroundColor Yellow
    Write-Host "💡 Tauri 앱을 실행하여 로그를 생성하거나, 수동으로 로그 파일을 생성하세요." -ForegroundColor Blue
    Write-Host ""
    $response = Read-Host "빈 로그 파일을 생성하시겠습니까? (y/N)"
    
    if ($response -match '^[Yy]$') {
        $logDir = Split-Path $SrcLog -Parent
        if (-not (Test-Path $logDir)) {
            New-Item -ItemType Directory -Path $logDir -Force | Out-Null
        }
        New-Item -ItemType File -Path $SrcLog -Force | Out-Null
        Write-Host "✅ 빈 로그 파일 생성됨: $SrcLog" -ForegroundColor Green
    }
    else {
        Write-Host "❌ 로그 덤프를 취소합니다." -ForegroundColor Red
        exit 1
    }
}

# Extract logs using PowerShell equivalent of tail
try {
    if ((Get-Item $SrcLog).Length -eq 0) {
        # Empty file
        "" | Out-File -FilePath $OutFile -Encoding UTF8
        Write-Host "✅ 빈 로그 파일에서 빈 출력 생성됨: $OutFile" -ForegroundColor Green
        Write-Host "📊 추출된 라인 수: 0" -ForegroundColor Cyan
        exit 0
    }
    else {
        # Extract last N lines (PowerShell equivalent of tail -n)
        $content = Get-Content $SrcLog -Encoding UTF8 -ErrorAction Stop
        if ($content.Count -le $Lines) {
            $content | Out-File -FilePath $OutFile -Encoding UTF8
            $extractedLines = $content.Count
        }
        else {
            $content[($content.Count - $Lines)..($content.Count - 1)] | Out-File -FilePath $OutFile -Encoding UTF8
            $extractedLines = $Lines
        }
        
        Write-Host "✅ 로그가 $SrcLog 에서 $OutFile 으로 저장되었습니다." -ForegroundColor Green
        Write-Host "📊 추출된 라인 수: $extractedLines" -ForegroundColor Cyan
        exit 0
    }
}
catch {
    Write-Host "❌ 로그 추출 실패: $_" -ForegroundColor Red
    exit 1
}