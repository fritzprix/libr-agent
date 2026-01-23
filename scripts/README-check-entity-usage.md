# Entity Usage Checker Scripts

Automated detection scripts for finding direct Entity and SQL query usage in the LibrAgent codebase, essential for tracking repository pattern migration progress.

## Overview

These scripts help identify locations where code directly uses SeaORM Entity objects or raw SQL queries instead of the repository pattern, which violates the architecture guidelines established in the refactoring plan.

## Files

- `check-entity-usage.ps1` - Windows PowerShell version
- `check-entity-usage.sh` - Linux/macOS Bash version

## Usage

### Windows (PowerShell)

```powershell
.\scripts\check-entity-usage.ps1
```

### Linux/macOS (Bash)

```bash
chmod +x scripts/check-entity-usage.sh
./scripts/check-entity-usage.sh
```

## What It Detects

The scripts search for three types of violations:

### 1. Direct Entity Usage

```rust
// ❌ BAD: Direct Entity access
settings::Entity::find_by_id(key)
mcp_server::Entity::find()
```

### 2. Entity Operations

```rust
// ❌ BAD: Direct Entity operations
assistant::Entity::find_by_id(id)
playbook::Entity::delete_by_id(id)
```

### 3. SQL Query References

```rust
// ❌ BAD: Direct SQL in code/comments
SELECT * FROM assistant WHERE id = ?
UPDATE session SET status = ?
```

## Excluded Locations

The following are intentionally excluded from detection:

- **Repository implementations** (`repositories/*.rs`) - These are allowed to use Entity internally
- **Entity definitions** (`entity.rs`, `entities.rs`) - Entity type definitions
- **Migration files** (`migration/`) - Database schema migrations
- **Build artifacts** (`target/`, `dist/`)
- **Dependencies** (`node_modules/`)

## Output Format

The script provides:

1. **Per-Table Breakdown**: Shows all violations for each table
2. **File Locations**: Exact file path and line number
3. **Violation Type**: Color-coded by type (Entity, Entity::find, SQL Query)
4. **Summary**: Total issues by table and migration phase

### Exit Codes

- `0` - No violations found (all tables migrated)
- `1` - Violations detected (migration needed)

## Migration Phases

### Phase 1 (DONE)

- ✅ `settings` - Settings repository pattern
- ✅ `mcp_server` - MCP server configuration repository
- ✅ `message_index_meta` - Message index metadata repository
- ✅ `message` - Message repository
- ✅ `session` - Session repository

**Note**: Phase 1 shows violations because repository implementations themselves use Entity internally (this is allowed and expected). External code should use the repository interfaces, not Entity directly.

### Phase 2 (TODO)

- ⏳ `assistant` - Assistant repository (23 violations)
- ⏳ `playbook` - Playbook repository (19 violations)
- ⏳ `knowledge` - Knowledge repository (3 violations)
- ✅ `planning_task` - No violations found
- ✅ `planning_reflection` - No violations found

## Integration with CI/CD

These scripts can be integrated into CI pipelines:

```yaml
# GitHub Actions example
- name: Check Entity Usage
  run: |
    if [ "$RUNNER_OS" == "Linux" ]; then
      ./scripts/check-entity-usage.sh
    else
      .\scripts\check-entity-usage.ps1
    fi
  shell: bash
```

## Understanding the Results

### Expected Violations

Repository implementations (`src-tauri/src/repositories/*.rs`) will show Entity usage - this is **correct and expected** because repositories encapsulate Entity operations.

### Actual Violations

Violations in other locations indicate code that should be migrated:

```rust
// src-tauri/src/commands/assistant_crud_commands.rs:64
// ❌ Should use repository instead
let assistant = assistant::Entity::find_by_id(&id)
```

**Should be refactored to:**

```rust
// ✅ Using repository pattern
let assistant = get_assistant_repository().get(&id).await?
```

## Related Documentation

- [Repository Pattern Refactoring Plan](../docs/refactoring/phase1-repository-pattern.md)
- [Phase 1 Completion Status](../REFACTORING_STATUS.md)
- [Coding Guidelines](.github/copilot-instructions.md)

## Maintenance

When adding new tables:

1. Update the `tables` array in both scripts
2. Specify the migration phase
3. Set initial status ("TODO" or "DONE")

Example:

```powershell
# PowerShell
@{ Name = "new_table"; Phase = "3"; Status = "TODO" }
```

```bash
# Bash
TABLE_PHASE["new_table"]="3"
TABLE_STATUS["new_table"]="○"
```

## Troubleshooting

### PowerShell: Script works but colors don't show

The script now uses native PowerShell colors (`Write-Host -ForegroundColor`) which work across all PowerShell versions. If colors still don't appear:

```powershell
# Ensure you're running PowerShell 5.1 or later
$PSVersionTable.PSVersion

# For PowerShell 7+, ANSI rendering is automatic
```

### Bash: Unicode characters display incorrectly

Make the script executable:

```bash
chmod +x scripts/check-entity-usage.sh
```

### False Positives

If a file is incorrectly flagged:

1. Check if it should be in the exclude list
2. Verify it's not a repository implementation
3. Update the exclude patterns if needed

## Contributing

When modifying these scripts:

1. Test both Windows and Linux versions
2. Ensure exit codes are correct
3. Keep detection patterns in sync
4. Update this README with any changes

## Examples

### Clean Migration (Phase 1 Complete)

```
--- message_index_meta (Phase 1) [DONE] ---
  [OK] No direct Entity or SQL usage found
```

### Violations Found (Phase 2 TODO)

```
--- assistant (Phase 2) [TODO] ---
  [!] src-tauri/src/commands/assistant_crud_commands.rs:64
      [Entity] let assistant = assistant::Entity::find_by_id(&id)

  [!] src-tauri/src/commands/assistant_crud_commands.rs:100
      [Entity::find] let assistants = assistant::Entity::find()
```

### Summary Output

```
================================================================
SUMMARY
================================================================

  [OK] settings (Phase 1 (DONE)): 0 issues
  [OK] mcp_server (Phase 1 (DONE)): 0 issues
  [!!] assistant (Phase 2): 23 issues

Total issues found: 23 (Migration needed)

================================================================
```
