# Dependency Analysis for Large File Refactoring

## Overview

This document analyzes the dependency relationships between the 5 target files for modularization:

1. `src/features/tools/BrowserToolProvider.tsx` (961 lines)
2. `src/features/chat/Chat.tsx` (939 lines)
3. `src-tauri/src/mcp/builtin/filesystem.rs` (842 lines)
4. `src/lib/db.ts` (841 lines)
5. `src-tauri/src/mcp.rs` (834 lines)

## Dependency Graph Analysis

### Frontend Dependencies (TypeScript/React)

#### BrowserToolProvider.tsx

**Imports:**

- React hooks: `useEffect`, `useRef`
- Local tools: `useBuiltInTool`, `ServiceContextOptions`
- Custom hooks: `useBrowserInvoker`, `useRustBackend`
- Types: `MCPTool`, `MCPResponse`, `ToolCall`
- Services: `getLogger`, Rust backend client functions
- External: `TurndownService`

**Used by:**

- `src/app/App.tsx` (main app composition)
- `src/features/tools/BrowserToolProvider.test.tsx` (testing)

**Risk Level: LOW** - Clear interface boundaries, no circular dependencies detected

#### Chat.tsx

**Imports:**

- UI components: `TerminalHeader`, various UI components from shadcn
- Context providers: `AssistantContext`, `SessionContext`, `ChatContext`, etc.
- Hooks: `useMCPServer`, `useRustBackend`, `useWebMCPServer`
- Models: `AttachmentReference`, `Message`
- Services: `getLogger`, `ContentStoreServer`
- Sub-components: `ToolsModal`, `MessageBubble`, `TimeLocationSystemPrompt`

**Used by:**

- `src/features/chat/index.tsx` (ChatRouter)
- `src/features/chat/GroupChatContainer.tsx`
- `src/features/chat/SingleChatContainer.tsx`
- Test files

**Risk Level: MEDIUM** - Heavy context dependencies, complex component tree

#### db.ts

**Imports:**

- Models: `Assistant`, `Group`, `Message`, `Session`
- External: `Dexie`, `Table`

**Used by:**

- `src/lib/web-mcp/modules/content-store/server.ts`
- `src/lib/web-mcp/modules/bm25/bm25-search-engine.ts`
- Multiple context files (AssistantContext, SessionContext, etc.)

**Risk Level: HIGH** - Core dependency used throughout the app, database migration concerns

### Backend Dependencies (Rust)

#### mcp.rs

**Imports:**

- External crates: `anyhow`, `log`, `rmcp`, `serde`, `tokio`, `uuid`
- Standard library: `std::collections::HashMap`, `std::sync::Arc`
- Local modules: `pub mod builtin`

**Used by:**

- `src-tauri/src/lib.rs` (main entry point)
- All builtin MCP server modules

**Risk Level: HIGH** - Core MCP functionality, affects all MCP operations

#### filesystem.rs

**Imports:**

- External crates: `async_trait`, `regex`, `serde_json`, `tokio`, `tracing`
- Parent modules: `BuiltinMCPServer`, schema builder utilities
- MCP types: `MCPError`, `MCPResponse`, `MCPTool`
- Services: `SecureFileManager`

**Used by:**

- `src-tauri/src/mcp/builtin/mod.rs` (builtin server registration)

**Risk Level: LOW** - Self-contained builtin server, clear interfaces

## Circular Dependency Risks

### Identified Potential Risks

1. **db.ts ↔ Context Files**: Database service is imported by many contexts, but contexts don't import each other circularly.

2. **mcp.rs ↔ builtin modules**: `mcp.rs` defines types used by builtin modules, but doesn't import them directly.

3. **Chat.tsx ↔ Context Dependencies**: Chat component imports multiple contexts, but contexts are designed with proper separation.

### Safe Refactoring Boundaries

Based on analysis, these modules can be safely split:

✅ **BrowserToolProvider.tsx**: Can be split into tool-specific modules without breaking dependencies
✅ **filesystem.rs**: Self-contained, can extract schema builders safely  
✅ **Chat.tsx**: Can extract hooks and components with proper interface design
⚠️ **db.ts**: Needs careful interface preservation during type/service separation
⚠️ **mcp.rs**: Core types must maintain API compatibility

## Recommended Refactoring Order

### Phase 1: Low Risk (BrowserToolProvider.tsx, filesystem.rs)

- Extract browser tools to individual modules
- Create schema builder utilities for filesystem operations
- No breaking changes to external APIs

### Phase 2: Medium Risk (Chat.tsx)

- Extract custom hooks (useChatState, etc.)
- Create sub-components (SessionFilesPopover, etc.)
- Maintain context API compatibility

### Phase 3: High Risk (mcp.rs, db.ts)

- Extract type definitions first
- Split services while preserving interfaces
- Require comprehensive testing

## Worker Compatibility Analysis

### Potential Issues

1. **Dynamic imports**: BrowserToolProvider modularization may affect tool loading in workers
2. **Database services**: Workers may need access to split db modules
3. **MCP type changes**: Workers using MCP types need stable interfaces

### Mitigation Strategies

1. **Preserve default exports** for dynamic tool loading
2. **Maintain barrel exports** in index files
3. **Use interface versioning** for breaking changes
4. **Test worker scenarios** after each phase

## Performance Impact Assessment

### Bundle Size Analysis (Pre-refactoring)

- Current largest chunk: ~2.4MB
- Target files contribute significantly to main bundle
- Code splitting opportunities identified

### Expected Improvements

- **Tree shaking**: Better elimination of unused code
- **Lazy loading**: Tool modules can be loaded on demand
- **Reduced rebuilds**: Smaller modules = faster incremental builds

### Monitoring Metrics

- Initial bundle size
- Tool loading latency
- Development build time
- Hot reload performance

## Testing Strategy

### Unit Test Requirements

- Each extracted module needs isolated tests
- Mock dependencies at module boundaries
- Preserve existing test coverage levels

### Integration Test Focus

- Database migration compatibility
- MCP tool registration flows
- Worker tool loading scenarios
- Context provider interactions

### Regression Test Checklist

- [ ] All browser tools function correctly
- [ ] Chat component renders and operates normally
- [ ] Database operations work across all contexts
- [ ] MCP servers start and communicate properly
- [ ] File system operations maintain security

## Success Criteria

### Code Quality Metrics

- [ ] No circular dependencies introduced
- [ ] All TypeScript strict mode compliance maintained
- [ ] ESLint rules pass without new violations
- [ ] Test coverage remains >= current levels

### Functional Requirements

- [ ] All existing features work unchanged
- [ ] No performance regressions
- [ ] Worker compatibility maintained
- [ ] Build and deployment succeed

### Maintainability Improvements

- [ ] File sizes reduced by target percentages
- [ ] Code duplication eliminated as planned
- [ ] Clear module boundaries established
- [ ] Documentation updated for new structure
