# Builtin Tools Standardization Plan

**Status**: Draft  
**Created**: 2026-01-24  
**Target Version**: 0.4.5  
**Estimated Effort**: Medium (3-5 days)

---

## Executive Summary

This plan addresses **inconsistent tool definition patterns** across LibrAgent's 10 builtin MCP servers. Currently, servers use 5 different patterns for defining tools, ranging from inline JSON to modular helper functions. This inconsistency:

- **Reduces maintainability**: Changes require understanding multiple patterns
- **Increases cognitive load**: Developers must learn different approaches per server
- **Hinders scalability**: Adding new tools requires context-specific knowledge
- **Creates technical debt**: No clear "correct" pattern for new servers

**Goal**: Establish and migrate to a **single, consistent pattern** based on the most successful implementations (Workspace, Content Store).

---

## Problem Analysis

### Current State: 5 Distinct Patterns

| **Pattern**             | **Servers**                        | **Characteristics**                                          | **Issues**                                     |
| ----------------------- | ---------------------------------- | ------------------------------------------------------------ | ---------------------------------------------- |
| **A: Inline JSON**      | Planning, Browser, MCP Manager, UI | Tools defined directly in `tools_static()` with JSON schemas | Monolithic, hard to maintain, poor readability |
| **B: Helper Functions** | Assistant, Knowledge               | Separate `create_*_tool()` functions in same file            | Better than inline, but still in single file   |
| **C: Modular Files**    | Workspace                          | Tools in dedicated `tools/*.rs` modules                      | Excellent organization, scalable               |
| **D: Internal Wrapper** | Playbook                           | Indirect via `tools_static_internal()`                       | Unnecessary indirection                        |
| **E: External Module**  | Content Store                      | Tools in `schemas.rs` module                                 | Clean separation, but inconsistent location    |

### Modularity Breakdown

```
❌ Low Modularity (4 servers)
   └── Planning, MCP Manager, UI, Browser
       • All tools in single mod.rs
       • 200-700 lines of tool definitions
       • Difficult to navigate and maintain

⚠️ Medium Modularity (3 servers)
   └── Assistant, Knowledge, Playbook
       • Helper functions in mod.rs
       • Better than inline, but still crowded
       • Operations split into separate files

✅ High Modularity (3 servers)
   └── Workspace, Content Store, Bootstrap
       • Dedicated tool/schema modules
       • Clear separation of concerns
       • Easy to add/modify tools
```

---

## Proposed Solution: Unified Modular Pattern

### Target Architecture

```
builtin/{server}/
├── mod.rs                    # Server implementation + BuiltinMCPServer trait
├── tools.rs                  # Tool definitions (NEW)
├── schemas.rs                # Schema builders (NEW, optional)
├── operations.rs             # Tool execution logic
├── queries.rs                # Read-only operations
└── helpers.rs                # Utility functions (optional)
```

### Standard Tool Definition Pattern

```rust
// ============================================
// mod.rs - Server Implementation
// ============================================
impl ServerName {
    pub fn tools_static() -> Vec<MCPTool> {
        tools::all_tools()  // Delegate to tools module
    }
}

// ============================================
// tools.rs - Tool Definitions (NEW FILE)
// ============================================
use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

pub fn all_tools() -> Vec<MCPTool> {
    vec![
        create_foo_tool(),
        create_bar_tool(),
        // Clear list of all tools
    ]
}

fn create_foo_tool() -> MCPTool {
    MCPTool {
        name: "fooTool".to_string(),
        title: Some("Foo Tool".to_string()),
        description: r#"Brief description

⚠️ CRITICAL WORKFLOW:
1. Step 1
2. Step 2

💡 Next Steps:
- Suggestion 1
- Suggestion 2"#.to_string(),
        input_schema: object_prop(
            vec![
                ("param1".to_string(), string_prop_required("Description")),
                ("param2".to_string(), integer_prop(Some(10), None, Some("Optional"))),
            ],
            vec!["param1".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

// ============================================
// schemas.rs - Reusable Schema Builders (OPTIONAL)
// ============================================
use crate::mcp::schema::JSONSchema;
use crate::mcp::utils::schema_builder::*;

pub fn foo_input_schema() -> JSONSchema {
    object_prop(
        vec![/* properties */],
        vec![/* required */],
        None,
    )
}
```

### Key Principles

1. **Single Responsibility**: `tools.rs` only defines tools, `operations.rs` executes them
2. **Schema Builders > Raw JSON**: Use `object_prop()`, `string_prop_required()` instead of `json!({})`
3. **Helper Functions**: One function per tool (`create_*_tool()`)
4. **Aggregator Pattern**: `all_tools()` returns complete list
5. **Optional Schemas Module**: For complex/reusable schemas only

---

## Migration Strategy

### Phase 1: High-Priority Servers (Week 1)

**Target**: Servers with worst maintainability issues

#### 1.1 Planning Server

- **Current**: 269 lines of inline JSON in `mod.rs`
- **Toolset**: 15 tools (goals, todos, scratchpad)
- **Migration Steps**:
  1. Create `planning/tools.rs`
  2. Extract inline `MCPTool` structs into `create_*_tool()` functions
  3. Convert JSON schemas to schema builder utilities
  4. Update `tools_static()` to call `tools::all_tools()`
  5. Test all 15 tools for schema compatibility

**Estimated Effort**: 4 hours

#### 1.2 MCP Manager Server

- **Current**: Inline JSON with complex query parameters
- **Toolset**: 6 tools (listServers, searchServer, etc.)
- **Migration Steps**:
  1. Create `mcp_manager/tools.rs`
  2. Extract tool definitions with pagination/filter schemas
  3. Standardize parameter naming (camelCase vs snake_case)
  4. Add workflow guidance to tool descriptions
  5. Test with server registry operations

**Estimated Effort**: 3 hours

#### 1.3 UI Server

- **Current**: Inline JSON + complex template rendering
- **Toolset**: 6 tools (promptUser, barChart, etc.)
- **Migration Steps**:
  1. Create `ui/tools.rs`
  2. Extract UI tool definitions
  3. Document template parameter schemas
  4. Add examples for each UI component
  5. Test interactive prompt rendering

**Estimated Effort**: 3 hours

### Phase 2: Medium-Priority Servers (Week 2)

**Target**: Partially modular servers needing cleanup

#### 2.1 Browser Server

- **Current**: Inline JSON + navigation/content modules
- **Toolset**: 10 tools (createSession, navigateToUrl, etc.)
- **Migration Steps**:
  1. Create `browser/tools.rs`
  2. Group related tools (session, navigation, interaction, content)
  3. Extract error handling patterns into shared helpers
  4. Update workflow guidance (timeout handling, 403/401 errors)
  5. Test browser automation scenarios

**Estimated Effort**: 4 hours

#### 2.2 Assistant Server

- **Current**: Helper functions in `mod.rs` (already modular)
- **Toolset**: 6 tools (CRUD operations)
- **Migration Steps**:
  1. Create `assistant/tools.rs`
  2. Move `create_*_assistant_tool()` functions
  3. Convert JSON schemas to builder utilities
  4. Keep operations.rs/queries.rs unchanged
  5. Verify CRUD test suite passes

**Estimated Effort**: 2 hours

#### 2.3 Knowledge Server

- **Current**: Helper functions in `mod.rs` (already modular)
- **Toolset**: 5 tools (save, read, delete, search, list)
- **Migration Steps**:
  1. Create `knowledge/tools.rs`
  2. Move `create_*_knowledge_tool()` functions
  3. Standardize search parameter schemas
  4. Update FTS5 search documentation
  5. Test knowledge base operations

**Estimated Effort**: 2 hours

### Phase 3: Low-Priority Servers (Week 3)

**Target**: Already well-structured servers needing minor alignment

#### 3.1 Playbook Server

- **Current**: Internal wrapper + builder utilities (good pattern)
- **Toolset**: 7 tools (create, select, list, etc.)
- **Migration Steps**:
  1. Create `playbook/tools.rs`
  2. Move `tools_static_internal()` content
  3. Remove unnecessary wrapper indirection
  4. Consolidate `create_tool_def()` calls
  5. Test playbook execution workflows

**Estimated Effort**: 2 hours

#### 3.2 Workspace Server

- **Current**: **Gold Standard** - highly modular with `tools/*.rs` modules
- **Action**: **Use as reference** - no migration needed
- **Recommendation**: Document pattern for future servers

**Estimated Effort**: 1 hour (documentation only)

#### 3.3 Content Store Server

- **Current**: External module pattern (`schemas.rs`)
- **Action**: Rename `schemas.rs` → `tools.rs` for consistency
- **Migration Steps**:
  1. Rename `content_store/schemas.rs` → `content_store/tools.rs`
  2. Update imports in `server.rs`
  3. Keep schema builder functions unchanged
  4. Test file upload/search operations

**Estimated Effort**: 1 hour

#### 3.4 Bootstrap Server

- **Current**: Helper functions + builder utilities (good pattern)
- **Toolset**: 2 tools (detectPlatform, getBootstrapGuide)
- **Migration Steps**:
  1. Create `bootstrap/tools.rs`
  2. Move `create_*_tool()` functions
  3. Keep platform detection logic in mod.rs
  4. Update platform-specific guidance
  5. Test cross-platform detection

**Estimated Effort**: 1.5 hours

---

## Schema Builder Standardization

### Current Issue: Mixed Schema Approaches

```rust
// ❌ Pattern A: Raw JSON (Planning, Browser, MCP Manager, UI)
input_schema: serde_json::from_value(json!({
    "type": "object",
    "properties": {
        "name": { "type": "string", "description": "..." }
    },
    "required": ["name"]
})).unwrap(),

// ⚠️ Pattern B: Partial Builders (Playbook, Bootstrap)
input_schema: object_prop(
    vec![("name".to_string(), string_prop_required("..."))],
    vec!["name".to_string()],
    None,
)

// ✅ Pattern C: Full Builders (Workspace tools module)
input_schema: create_file_input_schema(),  // Defined in schemas.rs
```

### Target: Unified Builder Pattern

**Utility Location**: `src-tauri/src/mcp/utils/schema_builder.rs` (already exists)

**Available Builders**:

```rust
// Basic types
string_prop_required(description)
string_prop(default, pattern, description)
integer_prop(default, minimum, description)
boolean_prop(description)
number_prop(default, min, max, description)

// Complex types
object_prop(properties, required, description)
array_schema(items_schema, description)
enum_prop(values, default, description)
enum_prop_required(values, description)

// Utility
object_schema(properties_map, required_vec)
```

**Migration Rule**: Replace all `serde_json::from_value(json!({...}))` with builder functions

---

## Testing Strategy

### Per-Server Validation

**For each migrated server**:

1. **Unit Tests**:

   ```bash
   cargo test --package libragent --lib mcp::builtin::{server}
   ```

2. **Tool Schema Validation**:

   ```rust
   #[test]
   fn test_all_tools_have_valid_schemas() {
       let tools = ServerName::tools_static();
       for tool in tools {
           assert!(tool.input_schema.validate().is_ok());
       }
   }
   ```

3. **Integration Tests**:
   ```bash
   cargo test --test integration_tests -- builtin_{server}
   ```

### End-to-End Validation

**After completing all phases**:

1. **Build Verification**:

   ```bash
   cargo check --all-targets
   cargo clippy -- -D warnings
   ```

2. **Frontend Integration**:

   ```bash
   pnpm lint
   pnpm build
   ```

3. **Agent Workflow Tests**:
   - Create test agent session for each server
   - Execute all tools with sample inputs
   - Verify error handling and guidance messages

4. **Full Validation Pipeline**:
   ```bash
   pnpm refactor:validate
   ```

---

## Success Criteria

### Technical Metrics

- ✅ **100% of servers** use `tools.rs` module pattern
- ✅ **0 inline JSON schemas** in server `mod.rs` files
- ✅ **All schemas** use builder utilities instead of raw JSON
- ✅ **Zero new clippy warnings** introduced
- ✅ **All existing tests pass** without modification

### Code Quality Metrics

- ✅ **Average lines per file** reduced by 30%
- ✅ **Tool definition readability** improved (peer review)
- ✅ **New tool creation time** reduced by 50% (developer survey)

### Documentation Requirements

- ✅ **Updated**: `docs/builtin-tools.md` with new patterns
- ✅ **Created**: `docs/guides/adding-new-builtin-tools.md` tutorial
- ✅ **Updated**: `.github/copilot-instructions.md` with standard pattern

---

## Risk Mitigation

### Risk 1: Schema Incompatibility

**Issue**: Builder utilities may not support all JSON schema features

**Mitigation**:

- Audit all existing schemas for unsupported features
- Extend `schema_builder.rs` with missing builders if needed
- Fallback: Keep complex schemas as JSON, document exceptions

### Risk 2: Breaking Changes

**Issue**: Schema format changes could break LLM tool calling

**Mitigation**:

- Use `serde_json::to_value()` to verify output equivalence
- Add regression tests comparing old/new schemas
- Test with actual LLM providers (OpenAI, Anthropic)

### Risk 3: Test Breakage

**Issue**: Existing tests may depend on tool structure

**Mitigation**:

- Run test suite after each server migration
- Fix tests incrementally before moving to next server
- Use feature flags to enable migration per-server

---

## Timeline & Resources

### Week 1: High-Priority Servers (Planning, MCP Manager, UI)

- **Days 1-2**: Planning Server migration
- **Days 3-4**: MCP Manager + UI migration
- **Day 5**: Testing and bug fixes

### Week 2: Medium-Priority Servers (Browser, Assistant, Knowledge)

- **Days 1-2**: Browser Server migration
- **Days 3-4**: Assistant + Knowledge migration
- **Day 5**: Testing and integration

### Week 3: Low-Priority Servers + Documentation

- **Days 1-2**: Playbook, Content Store, Bootstrap migration
- **Days 3-4**: Documentation updates and developer guide
- **Day 5**: Final validation and PR preparation

**Total Estimated Effort**: 24-30 hours (3-4 developer-days)

**Required Resources**:

- 1 Backend Developer (Rust)
- Code review from 1 Senior Developer
- QA testing across all builtin servers

---

## Follow-Up Actions

### Post-Migration

1. **Update Contribution Guidelines**:
   - Add "Tool Definition Standards" section
   - Require `tools.rs` module for new servers
   - Enforce schema builder usage in PR reviews

2. **Create GitHub Issue Templates**:
   - "New Builtin Server" template with standard structure
   - "Add Builtin Tool" template with checklist

3. **CI/CD Integration**:
   - Add linter rule: Detect inline `MCPTool` in `mod.rs`
   - Add test: Verify all servers have `tools.rs` module
   - Block PRs if standards violated

4. **Developer Training**:
   - Internal documentation session
   - Record video tutorial for onboarding
   - Update README with architecture diagram

---

## Appendix

### A. File Structure Comparison

**Before** (Planning Server):

```
builtin/planning/
├── mod.rs (374 lines - monolithic)
├── context.rs
├── goals.rs
├── scratchpad.rs
└── todos.rs
```

**After** (Planning Server):

```
builtin/planning/
├── mod.rs (120 lines - clean)
├── tools.rs (200 lines - tool definitions) ← NEW
├── schemas.rs (optional, if complex) ← NEW
├── context.rs
├── goals.rs
├── operations.rs (tool execution) ← RENAMED
└── queries.rs (read-only logic) ← RENAMED
```

### B. Schema Builder Reference

See: `src-tauri/src/mcp/utils/schema_builder.rs`

```rust
pub fn string_prop_required(description: &str) -> JSONSchema;
pub fn string_prop(default: Option<&str>, pattern: Option<&str>, description: Option<&str>) -> JSONSchema;
pub fn integer_prop(default: Option<i64>, minimum: Option<i64>, description: Option<&str>) -> JSONSchema;
pub fn boolean_prop(description: Option<&str>) -> JSONSchema;
pub fn object_prop(properties: Vec<(String, JSONSchema)>, required: Vec<String>, description: Option<&str>) -> JSONSchema;
pub fn array_schema(items: JSONSchema, description: Option<&str>) -> JSONSchema;
pub fn enum_prop(values: Vec<&str>, default: &str, description: Option<&str>) -> JSONSchema;
```

### C. Migration Checklist Template

**Per-Server Checklist**:

- [ ] Create `tools.rs` module
- [ ] Extract all tool definitions
- [ ] Convert JSON schemas to builders
- [ ] Update `tools_static()` to delegate
- [ ] Run unit tests
- [ ] Run integration tests
- [ ] Update inline documentation
- [ ] Code review
- [ ] Merge to dev branch

---

## Approval & Sign-Off

**Plan Author**: GitHub Copilot (AI Assistant)  
**Date**: 2026-01-24  
**Status**: Pending Review

**Required Approvals**:

- [ ] Tech Lead
- [ ] Backend Team
- [ ] QA Team

**Next Steps**:

1. Review and approve plan
2. Create tracking GitHub issue
3. Begin Phase 1 implementation
