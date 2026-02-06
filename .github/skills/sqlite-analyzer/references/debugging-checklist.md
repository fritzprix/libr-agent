# SQLite Database Debugging Checklist

## Pre-Investigation

### 1. Locate Database File

**Common Locations**:

- **Windows**: `%APPDATA%\{app_name}\database.db`
- **macOS**: `~/Library/Application Support/{app_name}/database.db`
- **Linux**: `~/.local/share/{app_name}/database.db`

**Find Database**:

```powershell
# PowerShell
Get-ChildItem -Path $env:APPDATA -Recurse -Filter "*.db" | Select-Object FullName

# Or use app-specific path construction
$appData = [Environment]::GetFolderPath('ApplicationData')
$dbPath = Join-Path $appData "com.fritzprix.libragent\libragent_v2.db"
```

```bash
# Bash
find ~ -name "*.db" 2>/dev/null

# Or use app-specific path
db_path="$HOME/Library/Application Support/com.fritzprix.libragent/libragent_v2.db"
```

---

### 2. Verify Database Accessibility

```powershell
# PowerShell
Test-Path $dbPath

# Check file size
(Get-Item $dbPath).Length / 1MB  # Size in MB
```

```bash
# Bash
ls -lh "$db_path"

# Check if readable
sqlite3 "$db_path" "PRAGMA integrity_check;"
```

---

## Investigation Workflow

### Step 1: Schema Inspection

**Goal**: Understand table structure, columns, and relationships

```bash
python scripts/analyze_schema.py <database_path> --output schema_report.md
```

**Manual queries**:

```sql
-- List all tables
SELECT name FROM sqlite_master WHERE type='table';

-- Get table structure
PRAGMA table_info(table_name);

-- Get foreign keys
PRAGMA foreign_key_list(table_name);

-- Get indexes
PRAGMA index_list(table_name);
```

**Expected Output**:

- Table names
- Column names and types
- Primary keys and foreign keys
- Index definitions

**Red Flags**:

- Missing indexes on foreign key columns
- No primary keys
- JSON columns without json_valid() constraints

---

### Step 2: Data Sampling

**Goal**: See actual data to understand content and formats

```bash
# Export as JSON
python scripts/export_sample.py <database_path> <table_name> --format json --limit 10

# Export as Markdown table
python scripts/export_sample.py <database_path> <table_name> --format markdown --limit 5
```

**Manual queries**:

```sql
-- Get sample rows
SELECT * FROM mcp_servers LIMIT 5;

-- Get distinct values for a column
SELECT DISTINCT status FROM processes;

-- Count rows
SELECT COUNT(*) FROM sessions;
```

**Expected Output**:

- Sample rows with actual data
- ID format verification (CUID2, UUID, etc.)
- NULL values identification

**Red Flags**:

- IDs that look like human-readable names
- NULL values in NOT NULL columns
- Inconsistent data formats

---

### Step 3: ID Format Validation

**Goal**: Detect name-as-id bugs (like MCPServerDto issue)

```bash
python scripts/compare_ids.py <database_path> --suspect-table mcp_servers
```

**Manual checks**:

```sql
-- Compare id and name columns
SELECT id, name FROM mcp_servers;

-- Check for duplicate IDs
SELECT id, COUNT(*) as count
FROM mcp_servers
GROUP BY id
HAVING count > 1;

-- Check for NULL IDs
SELECT * FROM mcp_servers WHERE id IS NULL;
```

**Expected Behavior**:

- `id` column: CUID2 format (e.g., `hiwqx3dj3tn82vt9amjysalj`)
- `name` column: Human-readable (e.g., `yfinance`)
- No duplicates, no NULLs

**Red Flags**:

- ID values match name values (e.g., both `"chess"`)
- IDs are simple strings instead of CUID2/UUID
- Backend returns `{"id":"chess","name":"chess"}` instead of `{"id":"hiwqx3dj3tn82vt9amjysalj","name":"chess"}`

---

### Step 4: Relationship Validation

**Goal**: Check foreign key integrity and find orphaned records

```bash
python scripts/find_orphans.py <database_path>
```

**Manual checks**:

```sql
-- Find orphaned sessions (assistant_id doesn't exist)
SELECT s.*
FROM sessions s
LEFT JOIN assistants a ON s.assistant_id = a.id
WHERE a.id IS NULL;

-- Find orphaned messages (session_id doesn't exist)
SELECT m.*
FROM messages m
LEFT JOIN sessions s ON m.session_id = s.id
WHERE s.id IS NULL;

-- Check foreign key constraints
PRAGMA foreign_key_check;
```

**Expected Outcome**:

- No orphaned records
- All foreign keys resolve to existing records

**Red Flags**:

- Orphaned records found
- Foreign key check returns violations
- CASCADE deletes not working

---

### Step 5: Backend DTO Mapping Verification

**Goal**: Verify Rust entity → DTO mapping is correct

**Check Entity Model**:

```rust
// src-tauri/src/entities/mcp_server.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "mcp_servers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,  // ← This is the CUID2
    pub name: String,  // ← This is human-readable
    // ...
}
```

**Check DTO Mapping**:

```rust
// src-tauri/src/commands/mcp_server_config_commands.rs
impl From<Model> for MCPServerDto {
    fn from(model: Model) -> Self {
        Self {
            id: model.id.clone(),  // ✅ CORRECT: Use actual ID
            // id: model.name.clone(),  // ❌ WRONG: Don't use name as ID
            name: model.name.clone(),
            // ...
        }
    }
}
```

**Testing**:

1. Query database directly: `SELECT id, name FROM mcp_servers LIMIT 1;`
2. Check backend response in frontend logs
3. Compare: Does `dto.id` match database `id` or `name`?

**Expected Result**:

- DTO `id` field contains CUID2 from database `id` column
- DTO `name` field contains human-readable name

**Bug Pattern**:

- DTO `id` field contains value from database `name` column
- Frontend filter by ID fails (looking for CUID2, but receives name string)

---

### Step 6: Frontend Data Flow Verification

**Goal**: Verify data arrives correctly from backend to UI

**Add Debug Logging**:

```typescript
// In service.ts
export async function getMCPServersByIds(ids: string[]): Promise<MCPServerDto[]> {
  const logger = getLogger('DBService');
  
  const allServers = await getAllMCPServers();
  logger.debug('🔍 Database getMCPServersByIds', {
    requestedIds: ids,
    allServersFromBackend: allServers.map(s => ({ id: s.id, name: s.name })),
  });
  
  const filtered = allServers.filter(server => ids.includes(server.id));
  logger.debug('🎯 Filtered servers', { count: filtered.length, servers: filtered });
  
  return filtered;
}
```

**Check Console Logs**:

1. Look for 🔍 log showing all servers from backend
2. Verify `id` field contains CUID2, not name
3. Look for 🎯 log showing filtered results
4. If count is 0, IDs don't match

**Expected Log Output**:

```
🔍 Database getMCPServersByIds {
  requestedIds: ["hiwqx3dj3tn82vt9amjysalj"],
  allServersFromBackend: [
    { id: "hiwqx3dj3tn82vt9amjysalj", name: "yfinance" },
    { id: "etag1t4gys3ub2gbq80hxj16", name: "chess" }
  ]
}
🎯 Filtered servers { count: 1, servers: [...] }
```

**Bug Pattern Log**:

```
🔍 Database getMCPServersByIds {
  requestedIds: ["hiwqx3dj3tn82vt9amjysalj"],
  allServersFromBackend: [
    { id: "yfinance", name: "yfinance" },  // ❌ id is name!
    { id: "chess", name: "chess" }
  ]
}
🎯 Filtered servers { count: 0, servers: [] }  // ❌ No matches
```

---

## Common Bug Patterns

### Bug 1: Name-as-ID Mapping

**Symptom**: IDs displayed in UI instead of names

**Root Cause**: Backend DTO uses `model.name` for `id` field

**Detection**:

```bash
python scripts/compare_ids.py database.db --suspect-table mcp_servers
```

**Fix**:

```rust
// BEFORE
id: model.name.clone(),

// AFTER
id: model.id.clone(),
```

---

### Bug 2: Missing Foreign Key Indexes

**Symptom**: Slow JOIN queries, high CPU usage

**Detection**:

```sql
-- Find tables with foreign keys but no indexes
SELECT name FROM sqlite_master WHERE type='table';
-- Then for each table:
PRAGMA foreign_key_list(table_name);
PRAGMA index_list(table_name);
-- Compare: If foreign key column not in index list, missing index
```

**Fix**:

```sql
CREATE INDEX idx_messages_session_id ON messages(session_id);
CREATE INDEX idx_sessions_assistant_id ON sessions(assistant_id);
```

---

### Bug 3: Array-in-Column Anti-Pattern

**Symptom**: Can't filter by array element, no foreign key constraints

**Detection**:

```sql
SELECT * FROM assistants WHERE mcp_server_ids LIKE '%server_id%';  -- Slow!
```

**Fix**: Migrate to junction table (see `references/common-patterns.md`)

---

### Bug 4: Orphaned Records

**Symptom**: Foreign key references non-existent records

**Detection**:

```bash
python scripts/find_orphans.py database.db
```

**Fix**:

```sql
-- Enable foreign keys
PRAGMA foreign_keys = ON;

-- Delete orphaned records
DELETE FROM messages
WHERE session_id NOT IN (SELECT id FROM sessions);

-- Or add CASCADE deletes
ALTER TABLE messages
ADD CONSTRAINT fk_messages_session
FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE;
```

---

## Post-Fix Verification

### 1. Build and Test

```powershell
# PowerShell
cargo build --manifest-path=src-tauri/Cargo.toml
pnpm tauri dev
```

### 2. Verify in UI

- Navigate to affected view (e.g., Assistant list)
- Check that names display correctly (not IDs)
- Check console logs for 🔍 🎯 debug messages

### 3. Database Integrity Check

```sql
PRAGMA integrity_check;
PRAGMA foreign_key_check;
```

---

## Prevention Checklist

- [ ] Always use `model.id` for entity IDs, never `model.name`
- [ ] Create indexes on all foreign key columns
- [ ] Enable foreign key constraints: `PRAGMA foreign_keys = ON;`
- [ ] Use junction tables for many-to-many relationships
- [ ] Add debug logging to DTO conversion functions
- [ ] Test with realistic data volumes (>1000 rows)
- [ ] Run schema analysis before and after migrations
- [ ] Document ID formats in entity models
- [ ] Use type-safe ID wrappers (e.g., `newtype` pattern)

---

## Tools Reference

| Tool | Purpose | Usage |
|------|---------|-------|
| `analyze_schema.py` | Generate schema report | `python analyze_schema.py db.db -o report.md` |
| `find_orphans.py` | Find orphaned records | `python find_orphans.py db.db` |
| `compare_ids.py` | Detect ID format bugs | `python compare_ids.py db.db -t table_name` |
| `export_sample.py` | Export sample data | `python export_sample.py db.db table -f json` |

---

## When to Escalate

**Escalate to team lead if**:

- Database corruption detected (integrity_check fails)
- Migration required affecting >100k rows
- Foreign key constraints prevent necessary operations
- Performance degradation >10x after schema change

**Document before escalating**:

- Schema report (from `analyze_schema.py`)
- Orphaned records count (from `find_orphans.py`)
- Sample data export (from `export_sample.py`)
- Backend logs showing DTO mapping
- Frontend logs showing filter results
