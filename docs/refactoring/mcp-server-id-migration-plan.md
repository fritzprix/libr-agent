# MCP Server ID Migration Plan

**Date**: February 6, 2026  
**Status**: Proposed  
**Priority**: HIGH - Affects data integrity and AI tool usability

---

## Executive Summary

The current MCP server management uses `name` as the PRIMARY KEY, which is conceptually mutable and creates foreign key integrity issues. Additionally, AI assistants cannot easily discover and reference MCP servers when creating/updating assistant configurations.

## Problems Identified

### 1. **Data Integrity Issue: Mutable Primary Key**

**Current Schema:**

```sql
CREATE TABLE mcp_servers (
    name TEXT PRIMARY KEY,          -- ❌ Mutable identifier used as PK
    config TEXT NOT NULL,
    tool_count INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
```

**Issues:**

- `name` is user-visible and conceptually changeable
- If renamed, breaks foreign key references in `assistants.config.mcpServerIds`
- No separate immutable ID for stable references

**Example Failure Scenario:**

```
1. MCP server created: name="my-filesystem"
2. Assistant references: mcpServerIds=["my-filesystem"]
3. User renames server: "my-filesystem" → "fs-server"
4. ❌ Assistant's reference is now BROKEN
5. ❌ No cascade update mechanism
```

### 2. **AI Usability Issue: Manual Name Extraction**

**Current Workflow (BROKEN):**

```
AI: I want to create an assistant with filesystem and github tools

Step 1: Call tool__list
Response: "Found 2 servers:
• filesystem [configured] (Type: stdio | Command: npx)
• github-api [configured] (Type: http | URL: https://...)"

Step 2: AI must PARSE text and EXTRACT names manually
mcpServerIds: ["filesystem", "github-api"]  // ❌ Error-prone

Problems:
- Text parsing required (not machine-readable)
- Typos and hallucinations common
- No validation until createAssistant call fails
```

### 3. **Missing Discovery Tools**

**What AI Assistants Need:**

- ✅ List all MCP servers with **stable IDs**
- ✅ Show which assistants use which servers
- ✅ Show tools provided by each server
- ✅ Machine-readable format (JSON in structured_content)

**Currently Missing:**

- No tool to show assistant ↔ MCP server associations
- No tool to list tools grouped by server
- tool\_\_list returns TEXT only (not AI-friendly)

---

## Proposed Solution

### Phase 1: Schema Migration (Breaking Change)

#### 1.1 Add Immutable UUID Column

**New Schema:**

```sql
CREATE TABLE mcp_servers (
    id TEXT PRIMARY KEY,            -- ✅ Immutable UUID (cuid2)
    name TEXT NOT NULL UNIQUE,      -- ✅ User-visible, mutable, UNIQUE constraint
    config TEXT NOT NULL,
    tool_count INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_mcp_servers_name ON mcp_servers(name);
```

**Migration Steps:**

```rust
// src-tauri/migration/src/m20260206_000001_add_mcp_server_id.rs

async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    // 1. Add new id column (nullable temporarily)
    manager.alter_table(
        Table::alter()
            .table(McpServers::Table)
            .add_column(ColumnDef::new(McpServers::Id).string().null())
            .to_owned()
    ).await?;

    // 2. Populate IDs for existing rows
    let db = manager.get_connection();
    let servers: Vec<mcp_server::Model> = mcp_server::Entity::find().all(db).await?;

    for server in servers {
        let id = cuid2::create_id();
        let mut active: mcp_server::ActiveModel = server.into();
        active.id = Set(Some(id));
        active.update(db).await?;
    }

    // 3. Make id NOT NULL
    manager.alter_table(
        Table::alter()
            .table(McpServers::Table)
            .modify_column(ColumnDef::new(McpServers::Id).string().not_null())
            .to_owned()
    ).await?;

    // 4. Drop old PRIMARY KEY and create new one
    // SQLite limitation: Cannot modify PRIMARY KEY directly
    // Must recreate table

    // Create new table with correct schema
    manager.create_table(
        Table::create()
            .table(McpServers::TableNew)
            .col(ColumnDef::new(McpServers::Id).string().not_null().primary_key())
            .col(ColumnDef::new(McpServers::Name).string().not_null().unique_key())
            .col(ColumnDef::new(McpServers::Config).string().not_null())
            .col(ColumnDef::new(McpServers::ToolCount).integer())
            .col(ColumnDef::new(McpServers::CreatedAt).big_integer().not_null())
            .col(ColumnDef::new(McpServers::UpdatedAt).big_integer().not_null())
            .to_owned()
    ).await?;

    // Copy data to new table
    manager.get_connection().execute_unprepared(
        "INSERT INTO mcp_servers_new SELECT id, name, config, tool_count, created_at, updated_at FROM mcp_servers"
    ).await?;

    // Drop old table and rename new table
    manager.drop_table(Table::drop().table(McpServers::Table).to_owned()).await?;
    manager.rename_table(
        Table::rename()
            .table(McpServers::TableNew, McpServers::Table)
            .to_owned()
    ).await?;

    // Create index on name
    manager.create_index(
        Index::create()
            .name("idx_mcp_servers_name")
            .table(McpServers::Table)
            .col(McpServers::Name)
            .to_owned()
    ).await?;

    // 5. CRITICAL: Migrate assistant configs from names to IDs
    let db = manager.get_connection();

    // Build name → id mapping from mcp_servers
    let servers: Vec<mcp_server::Model> = mcp_server::Entity::find().all(db).await?;
    let name_to_id: std::collections::HashMap<String, String> = servers
        .into_iter()
        .map(|s| (s.name.clone(), s.id.clone()))
        .collect();

    // Update each assistant's config
    let assistants: Vec<assistant::Model> = assistant::Entity::find().all(db).await?;

    for assistant in assistants {
        let mut config: serde_json::Value = serde_json::from_str(&assistant.config)
            .map_err(|e| DbErr::Custom(format!("Invalid JSON in assistant {}: {}", assistant.id, e)))?;

        // Migrate mcpServerIds from names to IDs
        if let Some(mcp_ids) = config.get_mut("mcpServerIds").and_then(|v| v.as_array_mut()) {
            let mut migrated = false;

            for id_value in mcp_ids.iter_mut() {
                if let Some(name) = id_value.as_str() {
                    if let Some(new_id) = name_to_id.get(name) {
                        *id_value = serde_json::json!(new_id);
                        migrated = true;
                    } else {
                        log::warn!(
                            "Assistant '{}' references unknown MCP server '{}' - removing reference",
                            assistant.name, name
                        );
                    }
                }
            }

            if migrated {
                // Remove invalid references (servers that don't exist)
                mcp_ids.retain(|v| name_to_id.values().any(|id| Some(id.as_str()) == v.as_str()));

                // Save updated config
                let mut active: assistant::ActiveModel = assistant.into();
                active.config = sea_orm::Set(serde_json::to_string(&config)?);
                active.update(db).await?;
            }
        }
    }

    Ok(())
}
```

#### 1.2 Update Entity Model

```rust
// src-tauri/src/entity/mcp_server.rs

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "mcp_servers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,              // ✅ NEW: Immutable UUID

    #[sea_orm(unique)]
    pub name: String,            // ✅ CHANGED: Now UNIQUE but not PK

    pub config: String,
    pub tool_count: Option<i32>,
    pub created_at: i64,
    pub updated_at: i64,
}
```

#### 1.3 Update Repository Methods

```rust
// src-tauri/src/repositories/mcp_server_repository.rs

#[async_trait]
pub trait MCPServerRepository: Send + Sync {
    /// Create with auto-generated ID
    async fn create(&self, name: &str, config: Value) -> Result<mcp_server::Model, DbError>;

    /// Get by ID (primary key)
    async fn get(&self, id: &str) -> Result<Option<mcp_server::Model>, DbError>;

    /// Get by name (for user lookups)
    async fn get_by_name(&self, name: &str) -> Result<Option<mcp_server::Model>, DbError>;

    /// Update by ID (allows name change)
    async fn update(&self, id: &str, name: Option<&str>, config: Option<Value>)
        -> Result<mcp_server::Model, DbError>;

    /// Delete by ID
    async fn delete(&self, id: &str) -> Result<(), DbError>;

    /// List all servers
    async fn list(&self) -> Result<Vec<mcp_server::Model>, DbError>;

    /// Update tool count after verification
    async fn update_tool_count(&self, id: &str, tool_count: i32) -> Result<(), DbError>;
}

impl SqliteMCPServerRepository {
    async fn create(&self, name: &str, config: Value) -> Result<mcp_server::Model, DbError> {
        let now = chrono::Utc::now().timestamp_millis();
        let id = cuid2::create_id();  // ✅ Auto-generate immutable ID

        // Check name uniqueness
        if self.get_by_name(name).await?.is_some() {
            return Err(DbError::DuplicateResource(format!(
                "MCP server with name '{}' already exists", name
            )));
        }

        let active = mcp_server::ActiveModel {
            id: Set(id.clone()),
            name: Set(name.to_string()),
            config: Set(config.to_string()),
            tool_count: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = active.insert(&self.db).await?;
        Ok(model)
    }

    async fn get_by_name(&self, name: &str) -> Result<Option<mcp_server::Model>, DbError> {
        use sea_orm::QueryFilter;
        let result = mcp_server::Entity::find()
            .filter(mcp_server::Column::Name.eq(name))
            .one(&self.db)
            .await?;
        Ok(result)
    }
}
```

### Phase 2: Update Tool Responses (AI-Friendly)

#### 2.1 Enhanced tool\_\_list Tool

**Goal: Make IDs immediately copy-pasteable with minimal parsing**

```rust
// src-tauri/src/mcp/builtin/mcp_manager/queries.rs

pub async fn list_servers(args: Value) -> Result<MCPResult, String> {
    let repo = get_mcp_server_repository();
    let models = repo.list().await.map_err(|e| format!("DB error: {}", e))?;

    // Pagination logic
    let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1);
    let page_size = args
        .get("pageSize")
        .and_then(|v| v.as_i64())
        .unwrap_or(20)
        .min(50) as usize;

    let total = models.len();
    let start = ((page - 1) * page_size as u64) as usize;
    let models_slice = if start >= total {
        Vec::new()
    } else {
        let end = (start + page_size).min(total);
        &models[start..end]
    };

    // Build AI-readable text with IDs prominently displayed
    let servers_text = models_slice
        .iter()
        .map(|model| {
            let config: MCPServerConfig = serde_json::from_str(&model.config)
                .unwrap_or_else(|_| panic!("Invalid config for {}", model.name));

            let transport_type = match config.transport {
                TransportConfig::Stdio { ref command, .. } => format!("stdio ({})", command),
                TransportConfig::Http { ref url, .. } => format!("http ({})", url),
            };

            let tool_count_str = model.tool_count
                .map(|c| format!(" [{} tools]", c))
                .unwrap_or_default();

            // ✅ Show both name and ID clearly
            format!(
                "• {}\n  ID: {}\n  Type: {}{}\n  Status: configured",
                model.name,
                model.id,          // ✅ ID on separate line for easy copying
                transport_type,
                tool_count_str
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let total_pages = (total as f64 / page_size as f64).ceil() as u64;

    // ✅ IMPROVED: Explicit example with actual IDs from response
    let example_ids = models_slice
        .iter()
        .take(2)
        .map(|m| format!("\"{}\"", m.id))
        .collect::<Vec<_>>()
        .join(", ");

    let hint = SuccessHint::new(
        format!(
            "📋 MCP Servers (Page {}/{})\n\n{}\n\n\
            💡 When creating an assistant, use the ID values:\n\n\
            Example:\n\
            mcpServerIds: [{}]\n\n\
            ⚠️ IMPORTANT: Use ID (not name). IDs are stable even if you rename the server.",
            page,
            total_pages,
            servers_text,
            example_ids  // ✅ Show actual IDs from this page
        ),
        vec![
            "Copy the ID line exactly (case-sensitive UUID)".to_string(),
            "Names can change, IDs cannot - always use IDs for references".to_string(),
            "Use registerServer to add new MCP servers".to_string(),
        ],
    );

    // ✅ structured_content contains machine-readable IDs
    let servers_json: Vec<Value> = models_slice
        .iter()
        .map(|m| {
            json!({
                "id": m.id,              // ✅ Stable identifier
                "name": m.name,          // Human-readable
                "toolCount": m.tool_count,
                "createdAt": m.created_at,
                "updatedAt": m.updated_at
            })
        })
        .collect();

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "servers": servers_json,
        "total": total,
        "page": page,
        "pageSize": page_size
    }))))
}
```

#### 2.2 New Tool: listServerAssociations

**Show which assistants use which servers**

```rust
// src-tauri/src/mcp/builtin/mcp_manager/queries.rs

pub async fn list_server_associations(_args: Value) -> Result<MCPResult, String> {
    let server_repo = get_mcp_server_repository();
    let assistant_repo = crate::repositories::SqliteAssistantRepository::new(/* db */);

    let servers = server_repo.list().await?;
    let assistants = assistant_repo.list().await?;

    if servers.is_empty() {
        return Ok(MCPResult::success(
            "No MCP servers registered yet.\n\n\
            💡 Use registerServer to add your first server."
        ));
    }

    // Build association map
    let mut associations = Vec::new();

    for server in servers {
        let mut using_assistants = Vec::new();

        for assistant in &assistants {
            let config: Value = serde_json::from_str(&assistant.config)?;
            if let Some(mcp_ids) = config.get("mcpServerIds").and_then(|v| v.as_array()) {
                if mcp_ids.iter().any(|id| id.as_str() == Some(&server.id)) {
                    using_assistants.push(assistant.name.clone());
                }
            }
        }

        associations.push(json!({
            "serverId": server.id,
            "serverName": server.name,
            "usedBy": using_assistants,
            "assistantCount": using_assistants.len()
        }));
    }

    // Build text output with clear ID visibility
    let text = associations
        .iter()
        .map(|assoc| {
            let name = assoc["serverName"].as_str().unwrap();
            let id = assoc["serverId"].as_str().unwrap();
            let count = assoc["assistantCount"].as_u64().unwrap();
            let assistants = assoc["usedBy"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ");

            if count == 0 {
                format!(
                    "• {}\n  ID: {}\n  Status: Not used by any assistant",
                    name, id
                )
            } else {
                format!(
                    "• {}\n  ID: {}\n  Used by {} assistant(s): {}",
                    name, id, count, assistants
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    Ok(SuccessHint::new(
        format!("📊 MCP Server Usage:\n\n{}", text),
        vec!["Use this to understand which servers are actively used".to_string()],
    )
    .to_mcp_result_with_data(Some(json!({ "associations": associations }))))
}
```

#### 2.3 Update Tool Schema Descriptions

**Clarify: name is user input (valid), id is system identifier (stable)**

```rust
// src-tauri/src/mcp/builtin/mcp_manager/tools.rs

pub fn create_register_server_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "name".to_string(),
        string_prop(
            None,
            None,
            Some("Unique display name for the MCP server (user-chosen).

NAMING RULES:
✅ Can be any descriptive string (e.g., 'My Filesystem', 'GitHub Personal')
✅ Must be unique across all registered servers
✅ You can change this later without breaking references
⚠️ System will auto-generate a stable ID for internal references

EXAMPLES:
  Correct: 'filesystem-project-a', 'github-api-work', 'local-db'
  Correct: 'My Custom Server', 'Acme Corp Tools'
  Incorrect: '' (empty)
  Incorrect: Duplicate of existing server name

💡 Use tool__list to see existing names"),
        ),
    );

    // ... rest of tool definition
}

pub fn create_update_server_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "id".to_string(),
        string_prop(
            None,
            None,
            Some("Server ID (immutable system identifier).

⚠️ WORKFLOW:
1. Call tool__list FIRST
2. Copy the exact 'ID' field (NOT the name)
3. Use that ID here

CORRECT: 'abc123-def456-uuid'
INCORRECT: 'filesystem' (that's the name, not ID)"),
        ),
    );

    props.insert(
        "name".to_string(),
        string_prop(
            None,
            None,
            Some("New display name for the server (optional).

RENAMING:
✅ You can change the name without breaking assistant configurations
✅ All assistants using this server keep working (they reference by ID)
⚠️ New name must be unique across all servers

EXAMPLE:
  Old: 'filesystem'
  New: 'filesystem-project-b'
  Result: Assistants continue working seamlessly"),
        ),
    );

    // ... rest of tool definition
}

pub fn create_assistant_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "mcpServerIds".to_string(),
        array_prop_with_desc(
            "Array of MCP server IDs (NOT names) to enable for this assistant.

⚠️ CRITICAL WORKFLOW:
1. Call tool__list FIRST
2. Extract the 'ID' field from each server (NOT the name)
3. Use those IDs in this parameter

CORRECT Example:
  mcpServerIds: [\"abc123-def456\", \"xyz789-uvw012\"]

INCORRECT Example:
  mcpServerIds: [\"filesystem\", \"github-api\"]  // ❌ Names won't work

💡 TIP: IDs are stable even if server name changes",
            false,
        ),
    );

    // ... rest of tool definition
}
```

### Phase 3: Add Validation Firewalls

```rust
// src-tauri/src/mcp/builtin/mcp_manager/operations.rs

pub async fn update_server(args: Value) -> Result<MCPResult, String> {
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: id".to_string())?;

    // ✅ FIREWALL: Verify ID exists before allowing updates
    let repo = get_mcp_server_repository();
    let existing = repo
        .get(id)
        .await
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or_else(|| {
            format!(
                "Server ID '{}' not found.\n\n\
                💡 Use tool__list to see valid IDs.",
                id
            )
        })?;

    // Get optional name update
    let new_name = args.get("name").and_then(|v| v.as_str());

    if let Some(name) = new_name {
        // Validate name uniqueness (except for current server)
        if let Ok(Some(other)) = repo.get_by_name(name).await {
            if other.id != id {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!("Server name '{}' already in use by another server", name),
                    vec![
                        "Server names must be unique across all servers".to_string(),
                        "Use tool__list to see existing names".to_string(),
                    ],
                    ToolGroup::MCPManager,
                ).to_mcp_result());
            }
        }
    }

    // Proceed with update...
    // (rest of update logic)
}

async fn validate_mcp_server_ids(
    db: &DatabaseConnection,
    server_ids: &[String],
) -> Result<(), String> {
    if server_ids.is_empty() {
        return Ok(());
    }

    let repo = SqliteMCPServerRepository::new(db.clone());

    // ✅ FIREWALL: Validate all IDs exist before accepting
    let mut valid_ids = Vec::new();
    let mut invalid_ids = Vec::new();

    for id in server_ids {
        match repo.get(id).await {
            Ok(Some(_)) => valid_ids.push(id),
            Ok(None) => invalid_ids.push(id),
            Err(e) => return Err(format!("DB error: {}", e)),
        }
    }

    if !invalid_ids.is_empty() {
        return Err(format!(
            "Invalid MCP server IDs: {}\n\n\
            ⚠️ Use tool__list to get valid IDs.\n\
            Remember: Use the 'ID' field, not the 'name' field.",
            invalid_ids.iter().map(|id| format!("'{}'", id)).collect::<Vec<_>>().join(", ")
        ));
    }

    Ok(())
}
```

### Phase 4: Frontend Migration

#### 4.1 Update TypeScript Types

```typescript
// src/models/chat.ts

export interface MCPServerEntity {
  id: string; // ✅ NEW: Immutable UUID (primary key)
  name: string; // ✅ User-visible identifier (mutable)
  isActive: boolean;
  transport: TransportConfig;
  authentication?: OAuthConfig;
  metadata?: ServerMetadata;
  toolCount?: number;
  createdAt: Date;
  updatedAt: Date;
}

export interface Assistant {
  id?: string;
  name: string;
  mcpServerIds?: string[]; // ✅ UNCHANGED: Still array of strings, but now UUIDs
  // ... rest unchanged
}
```

#### 4.2 Update DB Service Layer

```typescript
// src/lib/db/service.ts

getMCPServersByIds: async (ids: string[]): Promise<MCPServerEntity[]> => {
  const all = await mcpBackent.listMCPServers();
  return all.filter((s) => ids.includes(s.id));  // ✅ Already correct!
},
```

#### 4.3 Frontend DTO Conversion Strategy

**Current State Analysis:**

The frontend `MCPServerEntity` interface currently has both `id` and `name` fields, and **treats them as identical** (both equal to the database `name` column):

```typescript
// Current behavior (BEFORE migration):
interface MCPServerEntity {
  id: string; // Currently: database.name
  name: string; // Currently: database.name
  // ...
}
```

**After Migration:**

The backend will return distinct values:

```typescript
// After migration:
interface MCPServerEntity {
  id: string; // NEW: database.id (UUID, immutable)
  name: string; // database.name (label, mutable)
  // ...
}
```

**No Breaking Changes Required:**

Because the frontend code already uses `s.id` for filtering and references, the transition is seamless:

```typescript
// getMCPServersByIds - ALREADY CORRECT
return all.filter((s) => ids.includes(s.id));

// MCPServerContext.connectServersFromAssistant - ALREADY CORRECT
const serversToConnect = await getMCPServersByIds(mcpServerIds);

// Assistant config - ALREADY CORRECT
interface AssistantConfig {
  mcpServerIds: string[]; // Still array of strings, just UUIDs now
}
```

**Key Insight**: Because current code treats `id === name`, and migration changes backend to `id !== name`, frontend code continues working **without any code changes**—it simply starts receiving proper UUIDs in the `id` field instead of duplicated names.

---

## Migration Checklist

### Backend Changes

- [ ] Create migration file: `m20260206_000001_add_mcp_server_id.rs`
- [ ] Update `entity/mcp_server.rs` (add id, make name unique)
- [ ] Update `repositories/mcp_server_repository.rs` (all methods)
- [ ] Update `mcp/builtin/mcp_manager/operations.rs` (registerServer, updateServer, deleteServer)
- [ ] Update `mcp/builtin/mcp_manager/queries.rs` (list_servers, search_server)
- [ ] Add new tool: `listServerAssociations`
- [ ] Update `mcp/builtin/assistant/operations.rs` (validate_mcp_server_ids)
- [ ] Update tool descriptions in `mcp/builtin/assistant/tools.rs`
- [ ] Update tool descriptions in `mcp/builtin/mcp_manager/tools.rs`

### Frontend Changes

- [ ] Update TypeScript types (already mostly correct)
- [ ] Test assistant creation flow with new IDs
- [ ] Update any hardcoded name references
- [ ] Test MCP server rename functionality

### Testing

- [ ] Test migration on existing database
- [ ] Test creating assistant with server IDs
- [ ] Test renaming MCP server (assistant refs should persist)
- [ ] Test deleting MCP server (should fail if used by assistant)
- [ ] Test listServerAssociations tool
- [ ] Integration test: Full workflow from AI perspective

### Documentation

- [ ] Update API docs with new schema
- [ ] Update tool descriptions in copilot-instructions.md
- [ ] Create migration guide for users

---

## Breaking Changes

### For End Users

- **BREAKING**: Existing `mcpServerIds` in assistant configs will break
- **Migration**: Run migration script to convert name → ID mappings
- **Timeline**: Must complete before v0.4.0 release

### For AI Assistants

- **IMPROVED**: AI now gets stable IDs in tool responses
- **IMPROVED**: IDs visible in both text and structured_content
- **BREAKING**: Must update tool schemas to specify "ID not name"

---

## Rollout Strategy

### Phase 1 (v0.4.0-alpha)

- Schema migration
- Backend API changes
- Frontend updates
- Internal testing

### Phase 2 (v0.4.0-beta)

- Public beta with migration tool
- User feedback on AI assistant creation
- Documentation updates

### Phase 3 (v0.4.0-stable)

- Final release with full migration support
- Deprecated APIs removed

---

## Success Criteria

✅ **Data Integrity**: MCP servers can be renamed without breaking assistant references  
✅ **AI Usability**: AI can create assistants with 90%+ success rate on first try  
✅ **Discovery**: AI can query server associations and tool listings  
✅ **Backward Compat**: Existing assistants migrated successfully

---

## Alternatives Considered

### Alternative 1: Keep name as PK, add cascade updates

- ❌ Rejected: SQLite doesn't support CASCADE UPDATE for TEXT columns
- ❌ Rejected: Manual cascade in application layer is error-prone

### Alternative 2: Use name as PK, forbid renaming

- ❌ Rejected: Poor UX for users
- ❌ Rejected: Doesn't solve AI usability issue

### Alternative 3: Composite key (id + name)

- ❌ Rejected: Unnecessary complexity
- ❌ Rejected: Doesn't improve AI usability

---

## Conclusion

**RECOMMENDATION: Proceed with Phase 1-3 migration for v0.4.0**

This migration is **essential** for:

1. Data integrity (stable references)
2. AI usability (machine-readable IDs)
3. User experience (rename servers safely)
4. Future-proofing (OAuth flows, server sync, etc.)

**Estimated Effort**: 2-3 days  
**Risk Level**: Medium (schema migration always risky)  
**Impact**: High (fixes critical architectural flaw)
