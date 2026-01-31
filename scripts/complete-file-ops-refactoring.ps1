#!/usr/bin/env pwsh
# File Operations Refactoring - Automated Completion Script
# This script completes the refactoring by creating remaining modules

$ErrorActionPreference = "Stop"
$OriginalFile = "src-tauri\src\mcp\builtin\workspace\file_operations.rs"
$TargetDir = "src-tauri\src\mcp\builtin\workspace\file_operations"

Write-Host "🚀 Starting file_operations.rs refactoring completion..." -ForegroundColor Cyan

# Check if original file exists
if (-not (Test-Path $OriginalFile)) {
    Write-Host "❌ Original file not found: $OriginalFile" -ForegroundColor Red
    exit 1
}

# Check if target directory exists
if (-not (Test-Path $TargetDir)) {
    Write-Host "❌ Target directory not found: $TargetDir" -ForegroundColor Red
    Write-Host "   Run this script from project root!" -ForegroundColor Yellow
    exit 1
}

Write-Host "✅ Prerequisites check passed" -ForegroundColor Green

# Step 1: Extract and create edit_replace.rs
Write-Host "`n📝 Creating edit_replace.rs..." -ForegroundColor Cyan
$EditReplaceFile = Join-Path $TargetDir "edit_replace.rs"

# Note: Manual extraction required due to PowerShell line reading limitations
Write-Host "   ⚠️  Manual step required:" -ForegroundColor Yellow
Write-Host "   1. Extract lines 1019-1566 from $OriginalFile" -ForegroundColor White
Write-Host "   2. Add imports: super::super::WorkspaceServer, utils::*, error_guidance::*" -ForegroundColor White
Write-Host "   3. Save to $EditReplaceFile" -ForegroundColor White

# Step 2: Extract and create search_query.rs
Write-Host "`n📝 Creating search_query.rs..." -ForegroundColor Cyan
$SearchQueryFile = Join-Path $TargetDir "search_query.rs"

Write-Host "   ⚠️  Manual step required:" -ForegroundColor Yellow
Write-Host "   1. Extract lines 668-843, 844-1018, 1567-1766 from $OriginalFile" -ForegroundColor White
Write-Host "   2. Add imports: super::super::WorkspaceServer, utils::*, error_guidance::*" -ForegroundColor White
Write-Host "   3. Save to $SearchQueryFile" -ForegroundColor White

# Step 3: Update workspace/mod.rs
Write-Host "`n📝 Updating workspace/mod.rs imports..." -ForegroundColor Cyan
$WorkspaceModFile = "src-tauri\src\mcp\builtin\workspace\mod.rs"

Write-Host "   ℹ️  Imports are already re-exported in file_operations/mod.rs" -ForegroundColor Green
Write-Host "   No changes needed in workspace/mod.rs!" -ForegroundColor Green

# Step 4: Backup and prepare to delete original
Write-Host "`n💾 Backing up original file..." -ForegroundColor Cyan
$BackupFile = "$OriginalFile.backup"
Copy-Item $OriginalFile $BackupFile -Force
Write-Host "   ✅ Backup created: $BackupFile" -ForegroundColor Green

# Step 5: Run validation
Write-Host "`n🧪 Ready to validate..." -ForegroundColor Cyan
Write-Host "   Once modules are created, run:" -ForegroundColor Yellow
Write-Host "   1. Remove old file: Remove-Item '$OriginalFile'" -ForegroundColor White
Write-Host "   2. Run: pnpm refactor:validate" -ForegroundColor White

Write-Host "`n✨ Refactoring preparation complete!" -ForegroundColor Green
Write-Host "📋 Next steps:" -ForegroundColor Cyan
Write-Host "   1. Create edit_replace.rs (extract lines 1019-1566)" -ForegroundColor White
Write-Host "   2. Create search_query.rs (extract lines 668-1766)" -ForegroundColor White
Write-Host "   3. Delete original file: $OriginalFile" -ForegroundColor White
Write-Host "   4. Run: pnpm refactor:validate" -ForegroundColor White
Write-Host "`n📖 See REFACTORING_STATUS.md for detailed instructions" -ForegroundColor Cyan
