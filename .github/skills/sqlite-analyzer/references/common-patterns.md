# Common SQLite Database Patterns

## Entity Relationship Patterns

### 1. One-to-Many Relationship

**Pattern**: Assistant has many Sessions

```sql
CREATE TABLE assistants (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    system_prompt TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    assistant_id TEXT NOT NULL,
    name TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (assistant_id) REFERENCES assistants(id) ON DELETE CASCADE
);

CREATE INDEX idx_sessions_assistant_id ON sessions(assistant_id);
```

**Query Pattern**:

```sql
-- Get all sessions for an assistant
SELECT * FROM sessions WHERE assistant_id = 'asst_123';

-- Get assistant with session count
SELECT a.*, COUNT(s.id) as session_count
FROM assistants a
LEFT JOIN sessions s ON a.id = s.assistant_id
GROUP BY a.id;
```

---

### 2. Many-to-Many Relationship

**Pattern**: Assistants can access multiple MCP Servers

**❌ WRONG** (Array in column):

```sql
CREATE TABLE assistants (
    id TEXT PRIMARY KEY,
    mcp_server_ids TEXT  -- JSON array like '["server1", "server2"]'
);
```

**Problems**:
- Can't query efficiently
- Can't enforce foreign key constraints
- Can't join properly

**✅ CORRECT** (Junction table):

```sql
CREATE TABLE assistants (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE mcp_servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE assistant_mcp_servers (
    assistant_id TEXT NOT NULL,
    mcp_server_id TEXT NOT NULL,
    added_at INTEGER NOT NULL,
    PRIMARY KEY (assistant_id, mcp_server_id),
    FOREIGN KEY (assistant_id) REFERENCES assistants(id) ON DELETE CASCADE,
    FOREIGN KEY (mcp_server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE
);

CREATE INDEX idx_ams_assistant ON assistant_mcp_servers(assistant_id);
CREATE INDEX idx_ams_server ON assistant_mcp_servers(mcp_server_id);
```

**Query Pattern**:

```sql
-- Get all MCP servers for an assistant
SELECT m.*
FROM mcp_servers m
JOIN assistant_mcp_servers ams ON m.id = ams.mcp_server_id
WHERE ams.assistant_id = 'asst_123';

-- Get all assistants using a specific server
SELECT a.*
FROM assistants a
JOIN assistant_mcp_servers ams ON a.id = ams.assistant_id
WHERE ams.mcp_server_id = 'server_456';
```

---

### 3. Polymorphic Associations

**Pattern**: Content can be text, image, or resource

**Option 1: Single Table with Type Column**:

```sql
CREATE TABLE message_content (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('text', 'image', 'resource')),
    text_content TEXT,
    image_url TEXT,
    resource_uri TEXT,
    resource_mime_type TEXT,
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
);

CREATE INDEX idx_content_message ON message_content(message_id);
```

**Option 2: Separate Tables (Better for Complex Types)**:

```sql
CREATE TABLE text_content (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL,
    text TEXT NOT NULL,
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
);

CREATE TABLE image_content (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL,
    url TEXT NOT NULL,
    width INTEGER,
    height INTEGER,
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
);
```

---

### 4. Soft Delete Pattern

**Pattern**: Mark records as deleted without removing them

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    name TEXT,
    deleted_at INTEGER,  -- NULL = active, timestamp = deleted
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_sessions_deleted ON sessions(deleted_at);
```

**Query Pattern**:

```sql
-- Get active sessions only
SELECT * FROM sessions WHERE deleted_at IS NULL;

-- Soft delete
UPDATE sessions SET deleted_at = strftime('%s', 'now') WHERE id = 'sess_123';

-- Restore
UPDATE sessions SET deleted_at = NULL WHERE id = 'sess_123';

-- Permanent cleanup (run periodically)
DELETE FROM sessions 
WHERE deleted_at IS NOT NULL 
AND deleted_at < strftime('%s', 'now') - (30 * 24 * 60 * 60);  -- 30 days ago
```

---

### 5. Audit Trail Pattern

**Pattern**: Track all changes to records

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    name TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE session_audit (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    action TEXT NOT NULL CHECK(action IN ('created', 'updated', 'deleted')),
    old_value TEXT,  -- JSON snapshot
    new_value TEXT,  -- JSON snapshot
    changed_at INTEGER NOT NULL,
    changed_by TEXT,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX idx_audit_session ON session_audit(session_id);
CREATE INDEX idx_audit_changed_at ON session_audit(changed_at);
```

---

### 6. JSON Column Pattern

**Use Case**: Store flexible metadata or configuration

```sql
CREATE TABLE mcp_servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    config TEXT NOT NULL,  -- JSON string
    created_at INTEGER NOT NULL
);
```

**Query JSON** (SQLite 3.38+):

```sql
-- Extract JSON field
SELECT 
    id, 
    name,
    json_extract(config, '$.transport.type') as transport_type
FROM mcp_servers;

-- Filter by JSON field
SELECT * FROM mcp_servers
WHERE json_extract(config, '$.transport.type') = 'stdio';

-- Update JSON field
UPDATE mcp_servers
SET config = json_set(config, '$.tool_count', 5)
WHERE id = 'server_123';
```

---

### 7. Versioning Pattern

**Pattern**: Keep history of record versions

```sql
CREATE TABLE assistants (
    id TEXT PRIMARY KEY,
    version INTEGER NOT NULL DEFAULT 1,
    name TEXT NOT NULL,
    system_prompt TEXT,
    updated_at INTEGER NOT NULL
);

CREATE TABLE assistant_versions (
    id TEXT PRIMARY KEY,
    assistant_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    name TEXT NOT NULL,
    system_prompt TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (assistant_id) REFERENCES assistants(id) ON DELETE CASCADE,
    UNIQUE (assistant_id, version)
);

CREATE INDEX idx_versions_assistant ON assistant_versions(assistant_id, version);
```

**Version Management**:

```sql
-- Save current version before update
INSERT INTO assistant_versions (id, assistant_id, version, name, system_prompt, created_at)
SELECT gen_id(), id, version, name, system_prompt, updated_at
FROM assistants
WHERE id = 'asst_123';

-- Update main table
UPDATE assistants
SET version = version + 1, name = 'New Name', updated_at = strftime('%s', 'now')
WHERE id = 'asst_123';

-- Get version history
SELECT * FROM assistant_versions
WHERE assistant_id = 'asst_123'
ORDER BY version DESC;
```

---

## Common Timestamp Patterns

### Unix Timestamp (Recommended)

```sql
CREATE TABLE records (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
```

**Update Trigger**:

```sql
CREATE TRIGGER update_timestamp
AFTER UPDATE ON records
BEGIN
    UPDATE records SET updated_at = strftime('%s', 'now') WHERE id = NEW.id;
END;
```

---

## ID Generation Patterns

### CUID2 (Recommended)

- **Format**: 25 characters, lowercase alphanumeric
- **Example**: `hiwqx3dj3tn82vt9amjysalj`
- **Generation**: Use `cuid2` crate in Rust

```rust
use cuid2;

let id = cuid2::create_id();  // "hiwqx3dj3tn82vt9amjysalj"
```

### UUID v4

- **Format**: 36 characters with dashes
- **Example**: `550e8400-e29b-41d4-a716-446655440000`

```rust
use uuid::Uuid;

let id = Uuid::new_v4().to_string();
```

---

## Common Mistakes to Avoid

### ❌ Using `name` as `id`

```rust
// WRONG: Using entity name as ID
MCPServerDto {
    id: model.name.clone(),  // "yfinance"
    name: model.name.clone(),
}
```

**Problem**: Names are not unique and can change. IDs must be immutable and unique.

**Fix**:

```rust
MCPServerDto {
    id: model.id.clone(),    // "hiwqx3dj3tn82vt9amjysalj"
    name: model.name.clone(),
}
```

### ❌ Missing Indexes on Foreign Keys

```sql
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);
-- Missing: CREATE INDEX idx_messages_session ON messages(session_id);
```

**Problem**: JOIN queries will be slow without index on foreign key column.

### ❌ Storing Arrays as JSON in Columns

Use junction tables instead for proper querying and foreign key constraints.
