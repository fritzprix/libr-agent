#!/usr/bin/env pwsh
# check-phase1-completion.ps1
# Validates Phase 1 repository pattern migration completion
# Excludes repository implementations and test files from violation detection

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  Phase 1 Repository Pattern Migration - Completion Check" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Phase 1 tables
$phase1Tables = @(
    "settings",
    "mcp_server",
    "message_index_meta",
    "message",
    "session"
)

# Files that are ALLOWED to use Entity directly
$allowedFiles = @(
    "repositories/*.rs",           # Repository implementations
    "entity/*.rs",                  # Entity definitions
    "*_test.rs",                    # Unit tests
    "integration_tests.rs",         # Integration tests
    "*/tests/*.rs",                 # Test directories
    "migration/*.rs"                # Database migrations
)

$totalViolations = 0
$violationsByTable = @{}

foreach ($table in $phase1Tables) {
    Write-Host "Checking: $table" -ForegroundColor Yellow
    
    $violations = @()
    $entityPattern = "${table}::Entity"
    
    # Find all Rust files
    $rustFiles = Get-ChildItem -Path "src-tauri/src" -Recurse -Include "*.rs" -Exclude "entity.rs","entities.rs" |
        Where-Object { 
            $path = $_.FullName
            $isAllowed = $false
            
            # Check if file matches allowed patterns
            foreach ($pattern in $allowedFiles) {
                $globPattern = $pattern.Replace("/", "\")
                if ($path -like "*$globPattern*" -or $path -like "*\repositories\*" -or $path -like "*\entity\*" -or 
                    $path -like "*integration_tests*" -or $path -like "*_test.rs" -or $path -like "*\tests\*" -or
                    $path -like "*\migration\*") {
                    $isAllowed = $true
                    break
                }
            }
            
            -not $isAllowed
        }
    
    foreach ($file in $rustFiles) {
        $relativePath = $file.FullName.Replace("$PWD\", "").Replace("\", "/")
        $lines = Get-Content $file.FullName
        
        # Track test blocks to skip violations inside them
        $inTestBlock = $false
        $testBlockDepth = 0
        $lineNumber = 1
        
        foreach ($line in $lines) {
            # Detect start of test block
            if ($line -match '^\s*#\[cfg\(test\)\]' -or $line -match '^\s*#\[test\]') {
                $inTestBlock = $true
                $testBlockDepth = 0
            }
            
            # Track brace depth in test blocks
            if ($inTestBlock) {
                $openBraces = ([regex]::Matches($line, '\{')).Count
                $closeBraces = ([regex]::Matches($line, '\}')).Count
                $testBlockDepth += $openBraces - $closeBraces
                
                # Exit test block when all braces are closed
                if ($testBlockDepth -lt 0) {
                    $inTestBlock = $false
                    $testBlockDepth = 0
                }
            }
            
            # Only report violations outside test blocks
            if ($line -match $entityPattern -and -not $inTestBlock) {
                $violations += @{
                    File = $relativePath
                    Line = $lineNumber
                    Content = $line.Trim()
                }
            }
            
            $lineNumber++
        }
    }
    
    if ($violations.Count -gt 0) {
        Write-Host "  [!!] Found $($violations.Count) violation(s)" -ForegroundColor Red
        foreach ($v in $violations) {
            Write-Host "       $($v.File):$($v.Line)" -ForegroundColor DarkGray
            Write-Host "       $($v.Content)" -ForegroundColor DarkGray
        }
        $violationsByTable[$table] = $violations.Count
        $totalViolations += $violations.Count
    } else {
        Write-Host "  [OK] Clean - No violations" -ForegroundColor Green
        $violationsByTable[$table] = 0
    }
    Write-Host ""
}

# Summary
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "PHASE 1 COMPLETION STATUS" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

$allClean = $true
foreach ($table in $phase1Tables) {
    $count = $violationsByTable[$table]
    if ($count -eq 0) {
        Write-Host "  [OK] $table - Migrated to repository pattern" -ForegroundColor Green
    } else {
        Write-Host "  [!!] $table - $count external violations found" -ForegroundColor Red
        $allClean = $false
    }
}

Write-Host ""
if ($allClean) {
    Write-Host "RESULT: " -NoNewline
    Write-Host "Phase 1 COMPLETE" -ForegroundColor Green
    Write-Host ""
    Write-Host "All Phase 1 tables have been successfully migrated to the repository pattern." -ForegroundColor Green
    Write-Host "External code no longer directly accesses Entity objects for these tables." -ForegroundColor Green
} else {
    Write-Host "RESULT: " -NoNewline
    Write-Host "Phase 1 INCOMPLETE" -ForegroundColor Red
    Write-Host ""
    Write-Host "Total external violations: $totalViolations" -ForegroundColor Red
    Write-Host ""
    Write-Host "Action Required:" -ForegroundColor Yellow
    Write-Host "  - Migrate remaining Entity usages to repository pattern" -ForegroundColor Yellow
    Write-Host "  - Update commands/services to use repository interfaces" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

exit $(if ($allClean) { 0 } else { 1 })
