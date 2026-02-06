# SQLite Query Optimization Guide

## Performance Best Practices

### 1. Index Usage

**Problem**: Slow queries on large tables
**Solution**: Create appropriate indexes

```sql
-- Check if query uses index
EXPLAIN QUERY PLAN SELECT * FROM assistants WHERE id = 'abc123';

-- Create index for frequently queried columns
CREATE INDEX idx_assistants_name ON assistants(name);
CREATE INDEX idx_messages_session_id ON messages(session_id);

-- Composite index for multi-column queries
CREATE INDEX idx_messages_session_created 
ON messages(session_id, created_at DESC);
```

**Rule**: Index columns used in WHERE, JOIN, and ORDER BY clauses.

---

### 2. Query Analysis

**EXPLAIN QUERY PLAN** - Shows how SQLite executes query:

```sql
EXPLAIN QUERY PLAN
SELECT m.* 
FROM messages m
JOIN sessions s ON m.session_id = s.id
WHERE s.assistant_id = 'asst_123'
ORDER BY m.created_at DESC
LIMIT 50;
```

**Output Interpretation**:
- `SCAN TABLE` - Full table scan (slow, needs index)
- `SEARCH TABLE ... USING INDEX` - Uses index (fast)
- `USE TEMP B-TREE FOR ORDER BY` - Sorts in memory (acceptable for small results)

---

### 3. Pagination Best Practices

**❌ WRONG** (Slow for large offsets):
```sql
SELECT * FROM messages ORDER BY created_at DESC LIMIT 50 OFFSET 10000;
```

**✅ CORRECT** (Cursor-based pagination):
```sql
-- First page
SELECT * FROM messages ORDER BY created_at DESC LIMIT 50;

-- Next page (using last created_at from previous page)
SELECT * FROM messages 
WHERE created_at < ?last_created_at
ORDER BY created_at DESC 
LIMIT 50;
```

---

### 4. Foreign Key Performance

**Enable foreign key constraints**:
```sql
PRAGMA foreign_keys = ON;
```

**Check constraint violations** (expensive, run occasionally):
```sql
PRAGMA foreign_key_check;
PRAGMA foreign_key_check(table_name);
```

**Index foreign key columns**:
```sql
-- Speeds up JOIN operations
CREATE INDEX idx_messages_session_id ON messages(session_id);
CREATE INDEX idx_sessions_assistant_id ON sessions(assistant_id);
```

---

### 5. Aggregate Query Optimization

**COUNT() optimization**:
```sql
-- ❌ SLOW: Full table scan
SELECT COUNT(*) FROM messages WHERE session_id = 'sess_123';

-- ✅ FASTER: Use index on session_id
CREATE INDEX idx_messages_session_id ON messages(session_id);
SELECT COUNT(*) FROM messages WHERE session_id = 'sess_123';
```

**Statistics queries**:
```sql
-- Get approximate row count (fast)
SELECT seq FROM sqlite_sequence WHERE name='messages';

-- Get detailed statistics
ANALYZE;
SELECT * FROM sqlite_stat1;
```

---

### 6. Transaction Performance

**Batch inserts**:
```sql
BEGIN TRANSACTION;
INSERT INTO messages (session_id, role, content) VALUES ('s1', 'user', 'Hi');
INSERT INTO messages (session_id, role, content) VALUES ('s1', 'assistant', 'Hello');
-- ... more inserts
COMMIT;
```

**Batch size**: 1000-10000 inserts per transaction for optimal performance.

---

### 7. Common Anti-Patterns

#### Anti-Pattern 1: N+1 Query Problem

**❌ WRONG**:
```rust
// Fetches assistants
let assistants = get_all_assistants().await?;

// Executes N queries (one per assistant)
for assistant in assistants {
    let servers = get_mcp_servers_by_ids(&assistant.mcp_server_ids).await?;
    // ...
}
```

**✅ CORRECT**:
```rust
// Fetch all assistants
let assistants = get_all_assistants().await?;

// Collect all server IDs
let all_server_ids: Vec<String> = assistants
    .iter()
    .flat_map(|a| a.mcp_server_ids.clone())
    .collect();

// Single query for all servers
let all_servers = get_mcp_servers_by_ids(&all_server_ids).await?;
```

#### Anti-Pattern 2: SELECT *

**❌ WRONG**:
```sql
SELECT * FROM messages WHERE session_id = 'sess_123';
```

**✅ CORRECT**:
```sql
SELECT id, role, content, created_at 
FROM messages 
WHERE session_id = 'sess_123';
```

**Reason**: Only retrieve columns you need. Reduces I/O and memory usage.

---

### 8. Query Profiling

**Measure query performance**:
```sql
-- Turn on timing
.timer on

-- Run query
SELECT COUNT(*) FROM messages;

-- Check execution time
```

**Use ANALYZE for query planner**:
```sql
ANALYZE;
-- Now query planner has better statistics
```

---

## Performance Checklist

- [ ] Indexes created for frequently queried columns
- [ ] Foreign key columns indexed
- [ ] Composite indexes for multi-column WHERE clauses
- [ ] Avoid SELECT * - specify columns
- [ ] Use cursor-based pagination for large datasets
- [ ] Batch inserts in transactions
- [ ] Avoid N+1 query problems
- [ ] Run ANALYZE periodically
- [ ] Monitor query plans with EXPLAIN

---

## Benchmarking Tips

**Test with realistic data volume**:
```sql
-- Check row counts
SELECT 
    name AS table_name,
    (SELECT COUNT(*) FROM sqlite_master sm WHERE sm.name = m.name) AS row_count
FROM sqlite_master m
WHERE type='table';
```

**Simulate production load**: Insert 10k-100k rows for accurate performance testing.

**Profile before optimizing**: Measure first, then optimize the slowest queries.
