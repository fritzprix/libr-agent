# Script to find files with more than 500 lines in the codebase
# Excludes node_modules, dist, and other generated/dependency directories

# ANSI color codes for PowerShell
$Red = "`e[0;31m"
$Yellow = "`e[1;33m"
$Green = "`e[0;32m"
$Blue = "`e[0;34m"
$NC = "`e[0m" # No Color

Write-Host "🔍 Finding files with more than 500 lines..."
Write-Host "=========================================="

# Define extensions to search for
$extensions = @('*.ts', '*.tsx', '*.js', '*.jsx', '*.rs')

# Define paths to exclude
$excludePaths = @(
    '*\node_modules\*',
    '*\dist\*',
    '*\target\*',
    '*\build\*',
    '*\.next\*',
    '*\.nuxt\*',
    '*\coverage\*'
)

# Define search directories
$searchDirs = @('.\src', '.\src-tauri\src', '.\docs')

# Collect all files with line counts
$files = @()
$totalLines = 0

foreach ($dir in $searchDirs) {
    if (Test-Path $dir) {
        Get-ChildItem -Path $dir -Recurse -Include $extensions -File -ErrorAction SilentlyContinue | 
            Where-Object { 
                $path = $_.FullName
                $exclude = $false
                foreach ($excludePath in $excludePaths) {
                    if ($path -like $excludePath) {
                        $exclude = $true
                        break
                    }
                }
                -not $exclude
            } | ForEach-Object {
                $lineCount = (Get-Content $_.FullName -ErrorAction SilentlyContinue | Measure-Object -Line).Lines
                $totalLines += $lineCount
                
                if ($lineCount -gt 500) {
                    $files += [PSCustomObject]@{
                        Lines = $lineCount
                        Path = $_.FullName -replace [regex]::Escape((Get-Location).Path + '\'), ''
                    }
                }
            }
    }
}

# Sort files by line count (descending) and display with colors
$files | Sort-Object -Property Lines -Descending | ForEach-Object {
    $lines = $_.Lines
    $path = $_.Path
    
    if ($lines -ge 1000) {
        Write-Host "${Red}🔴 $($lines.ToString().PadLeft(6)) lines: $path${NC}"
    } elseif ($lines -ge 800) {
        Write-Host "${Yellow}🟡 $($lines.ToString().PadLeft(6)) lines: $path${NC}"
    } else {
        Write-Host "${Green}🟢 $($lines.ToString().PadLeft(6)) lines: $path${NC}"
    }
}

Write-Host ""
Write-Host "=========================================="
Write-Host "✅ Scan complete!"
Write-Host ""
Write-Host "${Blue}📊 📈 TOTAL LINES IN CODEBASE: $totalLines${NC}"
Write-Host "📊 Total files with >500 lines: $($files.Count)"

# Show color legend
Write-Host ""
Write-Host "🎨 Color Legend:"
Write-Host "  ${Red}🔴 1000+ lines${NC}"
Write-Host "  ${Yellow}🟡 800-999 lines${NC}"
Write-Host "  ${Green}🟢 500-799 lines${NC}"
Write-Host "  ${Blue}📈 Total lines (summary)${NC}"
