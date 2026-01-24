#!/usr/bin/env pwsh
# check-entity-usage.ps1
# Detects direct Entity usage and SQL queries for repository pattern migration
# Usage: .\scripts\check-entity-usage.ps1

$ErrorActionPreference = "Stop"

# Enable VT100 for ANSI colors in Windows Terminal
if ($PSVersionTable.PSVersion.Major -ge 7) {
    $PSStyle.OutputRendering = 'Ansi'
}

# Tables to check with their migration status
$tables = @(
    @{ Name = "settings"; Phase = "1 (DONE)"; Status = "DONE" },
    @{ Name = "mcp_server"; Phase = "1 (DONE)"; Status = "DONE" },
    @{ Name = "message_index_meta"; Phase = "1 (DONE)"; Status = "DONE" },
    @{ Name = "message"; Phase = "1 (DONE)"; Status = "DONE" },
    @{ Name = "session"; Phase = "1 (DONE)"; Status = "DONE" },
    @{ Name = "assistant"; Phase = "2"; Status = "TODO" },
    @{ Name = "playbook"; Phase = "2"; Status = "TODO" },
    @{ Name = "knowledge"; Phase = "2"; Status = "TODO" },
    @{ Name = "planning_task"; Phase = "2"; Status = "TODO" },
    @{ Name = "planning_reflection"; Phase = "2"; Status = "TODO" }
)

# Directories to search
$searchDirs = @("src-tauri/src")

# Directories to exclude
$excludeDirs = @(
    "target",
    "node_modules",
    ".git",
    "dist",
    "migration"
)

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  Entity & SQL Query Usage Detector (Repository Pattern)" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

$totalIssues = 0
$issuesByTable = @{}

foreach ($table in $tables) {
    $tableName = $table.Name
    $phase = $table.Phase
    $status = $table.Status
    
    $statusSymbol = if ($status -eq "DONE") { "[DONE]" } else { "[TODO]" }
    Write-Host "--- $tableName (Phase $phase) $statusSymbol ---" -ForegroundColor Blue
    
    $issues = @()
    
    # Pattern 1: Direct Entity usage (e.g., settings::Entity)
    $entityPattern = "${tableName}::Entity"
    
    # Pattern 2: Entity find operations
    $findPatterns = @(
        "${tableName}::Entity::find",
        "${tableName}::Entity::find_by_id",
        "${tableName}::Entity::insert",
        "${tableName}::Entity::update",
        "${tableName}::Entity::delete"
    )
    
    # Pattern 3: Direct SQL table references in strings
    $sqlPatterns = @(
        "SELECT.*FROM\s+$tableName",
        "INSERT\s+INTO\s+$tableName",
        "UPDATE\s+$tableName",
        "DELETE\s+FROM\s+$tableName"
    )
    
    # Search for Entity usage
    foreach ($dir in $searchDirs) {
        if (Test-Path $dir) {
            # Build exclude pattern
            $excludePattern = ($excludeDirs | ForEach-Object { "-not -path '*/$_/*'" }) -join " "
            
            # Search for direct Entity references
            $entityFiles = Get-ChildItem -Path $dir -Recurse -Include "*.rs" -Exclude "entity.rs","entities.rs","mod.rs" | 
                Where-Object { 
                    $path = $_.FullName
                    $exclude = $false
                    foreach ($excludeDir in $excludeDirs) {
                        if ($path -like "*\$excludeDir\*") {
                            $exclude = $true
                            break
                        }
                    }
                    -not $exclude
                }
            
            foreach ($file in $entityFiles) {
                $relativePath = $file.FullName.Replace("$PWD\", "").Replace("\", "/")
                $content = Get-Content $file.FullName -Raw
                
                # Check for Entity usage
                if ($content -match $entityPattern) {
                    $lineNumber = 1
                    $lines = Get-Content $file.FullName
                    foreach ($line in $lines) {
                        if ($line -match $entityPattern) {
                            $issues += @{
                                Type = "Entity"
                                File = $relativePath
                                Line = $lineNumber
                                Content = $line.Trim()
                            }
                        }
                        $lineNumber++
                    }
                }
                
                # Check for find operations
                foreach ($pattern in $findPatterns) {
                    if ($content -match [regex]::Escape($pattern)) {
                        $lineNumber = 1
                        $lines = Get-Content $file.FullName
                        foreach ($line in $lines) {
                            if ($line -match [regex]::Escape($pattern)) {
                                $issues += @{
                                    Type = "Entity::find"
                                    File = $relativePath
                                    Line = $lineNumber
                                    Content = $line.Trim()
                                }
                            }
                            $lineNumber++
                        }
                    }
                }
                
                # Check for SQL queries
                foreach ($sqlPattern in $sqlPatterns) {
                    if ($content -match $sqlPattern) {
                        $lineNumber = 1
                        $lines = Get-Content $file.FullName
                        foreach ($line in $lines) {
                            if ($line -match $sqlPattern) {
                                $issues += @{
                                    Type = "SQL Query"
                                    File = $relativePath
                                    Line = $lineNumber
                                    Content = $line.Trim()
                                }
                            }
                            $lineNumber++
                        }
                    }
                }
            }
        }
    }
    
    # Display results
    if ($issues.Count -gt 0) {
        foreach ($issue in $issues) {
            $typeColor = switch ($issue.Type) {
                "Entity" { "Red" }
                "Entity::find" { "Yellow" }
                "SQL Query" { "Magenta" }
                default { "White" }
            }
            
            Write-Host "  [!] $($issue.File):$($issue.Line)" -ForegroundColor $typeColor
            Write-Host "      [$($issue.Type)] $($issue.Content)" -ForegroundColor DarkGray
        }
        
        $issuesByTable[$tableName] = $issues.Count
        $totalIssues += $issues.Count
    } else {
        Write-Host "  [OK] No direct Entity or SQL usage found" -ForegroundColor Green
        $issuesByTable[$tableName] = 0
    }
    
    Write-Host ""
}

# Summary
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "SUMMARY" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

foreach ($table in $tables) {
    $tableName = $table.Name
    $count = $issuesByTable[$tableName]
    $phase = $table.Phase
    
    if ($count -eq 0) {
        Write-Host "  [OK] $tableName (Phase $phase): $count issues" -ForegroundColor Green
    } else {
        Write-Host "  [!!] $tableName (Phase $phase): $count issues" -ForegroundColor Red
    }
}

Write-Host ""
Write-Host "Total issues found: " -NoNewline
if ($totalIssues -eq 0) {
    Write-Host "$totalIssues " -ForegroundColor Green -NoNewline
    Write-Host "(All tables migrated to repository pattern!)" -ForegroundColor Green
} elseif ($totalIssues -lt 10) {
    Write-Host "$totalIssues " -ForegroundColor Yellow -NoNewline
    Write-Host "(Almost there!)" -ForegroundColor Yellow
} else {
    Write-Host "$totalIssues " -ForegroundColor Red -NoNewline
    Write-Host "(Migration needed)" -ForegroundColor Red
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

exit $(if ($totalIssues -eq 0) { 0 } else { 1 })
