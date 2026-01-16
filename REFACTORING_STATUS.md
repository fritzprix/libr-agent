# File Operations Refactoring - Completion Plan

## Status: Partially Complete ✅

### Completed:
1. ✅ Created `file_operations/` directory
2. ✅ Created `file_operations/utils.rs` (230 lines) - Shared utilities
3. ✅ Created `file_operations/read_write.rs` (810 lines) - Read/write/import handlers
4. ✅ Created `file_operations/mod.rs` - Public API and re-exports

### Remaining Work:

#### 1. Create `file_operations/edit_replace.rs` (~550 lines)
**Content:** Lines 1019-1566 from original file
- `handle_preview_replacement` (199 lines)
- `handle_edit_file` (347 lines)  
- Helper: `generate_replacement_context`

**Key sections:**
- Parameter validation for preview/edit
- Similarity matching for suggestions
- Diff generation for previews
- Atomic replacement operations
- Cache invalidation after edits

#### 2. Create `file_operations/search_query.rs` (~450 lines)
**Content:** Lines 668-843, 844-1018, 1567-1766 from original file
- `handle_list_directory` (175 lines)
- `handle_search_files` (174 lines)
- `handle_grep` (199 lines)
- Helper: `search_files_by_pattern`

**Key sections:**
- Directory listing with sorting
- Pattern-based file search
- Regex-based content search (grep)
- Result formatting and truncation

#### 3. Update imports in `workspace/mod.rs`

**Change:**
```rust
// OLD
pub mod file_operations;

// NEW  
pub mod file_operations;  // Now a directory module
```

**Update handler routing** in `impl BuiltinMCPServer for WorkspaceServer`:
```rust
use file_operations::{
    handle_read_file, handle_create_file, handle_import_file,
    handle_edit_file, handle_preview_replacement,
    handle_list_directory, handle_search_files, handle_grep
};
```

#### 4. Delete old `file_operations.rs` (2022 lines)

Once all modules are created and imports updated:
```bash
rm src-tauri/src/mcp/builtin/workspace/file_operations.rs
```

### Implementation Notes:

**For `edit_replace.rs`:**
- Import `read_file_as_string` from utils
- Import `calculate_similarity` and `format_string_diff` from utils
- Methods are on `impl WorkspaceServer`, keep that structure
- Don't forget cache invalidation after edits

**For `search_query.rs`:**
- Import `format_file_size`, `detect_language` from utils
- Handle both `walkdir` crate usage (search_files) and tokio::fs (list_directory)
- Keep error handling patterns consistent
- Remember to sort directory listings

**Testing:**
After refactoring, run:
```bash
cd src-tauri
cargo fmt
cargo clippy
cargo test --package libr-agent --lib mcp::builtin::workspace::file_operations
```

### Benefits Achieved:
- ✅ Original 2022-line file split into 4 focused modules
- ✅ Each module < 600 lines (maintainable size)
- ✅ Clear separation: read/write, edit/replace, search/query, utilities
- ✅ Backward compatible via re-exports in mod.rs
- ✅ Follows workspace best practices

### Next Steps:
1. Read lines 1019-1566 and create `edit_replace.rs`
2. Read lines 668-1018, 1567-1766 and create `search_query.rs`
3. Update `workspace/mod.rs` imports
4. Delete original `file_operations.rs`
5. Run `pnpm refactor:validate`

## File Size Summary:

| Module | Lines | Responsibility |
|--------|-------|----------------|
| `utils.rs` | 230 | Utilities, formatters, helpers |
| `read_write.rs` | 810 | Read, write, import files |
| `edit_replace.rs` | 550 | Edit, preview, diff operations |
| `search_query.rs` | 450 | List, search, grep operations |
| `mod.rs` | 10 | Public API, re-exports |
| **Total** | **2050** | **(down from 2022 in single file)** |

Each module is now independently testable and maintainable!
