# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

No changes yet.

## [0.3.5] - 2025-11-20

### 🐛 Fixed

- **Circular Dependency**: Resolved a circular dependency in the database module by extracting `LocalDatabase` into a separate file (`src/lib/db/database.ts`). This fixes the `TypeError` during test initialization.
- **Server Lookup**: Fixed case-insensitive server lookup in `McpServerService` to correctly find servers regardless of name casing.

## [0.3.2] - 2025-11-19

### 🛠️ Improvements

- **Windows Process Execution Improvements**: Enhanced file output streaming with explicit buffering and flushing to ensure complete stderr/stdout capture on Windows. Added detailed logging of Windows environment variables and output file sizes for better diagnostics. Implemented a short delay after process completion to ensure file system synchronization.
- **Process Output Streaming Enhancements**: Switched to `tokio::io::BufWriter` with explicit flushing for more reliable file writes. Added a new hybrid streaming method (`spawn_and_stream_hybrid`) that streams process output to both files and in-memory buffers, supporting broadcast channels and circular buffers for async and long-running processes.
- **Quote Normalization Fix**: Fixed Windows command normalization logic to avoid altering quotes, preventing issues with nested quotes in inline Python and Node.js commands.
- **Token Budget Improvements**: Enhanced token budget calculations to account for system prompts and tools JSON when selecting messages within context windows, allowing for more accurate prompt management.
- **Ollama Dependency Update**: Updated Ollama dependency from 0.6.0 to 0.6.3 for improved model support and bug fixes.
- **Default Context Window**: Increased default context window fallback from 4096 to 32768 tokens to better support modern large language models.

## [0.3.1] - 2025-11-18

### 🐛 Fixed

#### Windows PowerShell Error Output Capture

- **Error Capture Fix**: Fixed `builtin_workspace__execute_windows_cmd` tool to properly capture PowerShell error messages in stderr
  - Previously, failed PowerShell commands (e.g., `Remove-Item`) returned `exit_code: 1` but empty stdout/stderr
  - Now wraps PowerShell commands with try-catch and `$ErrorActionPreference = 'Stop'` to redirect errors to stderr
  - Error messages now include exception details and stack traces for better debugging
  - Affects: `execute_windows_cmd` tool on Windows platform
- **Testing**: Added unit tests for PowerShell error wrapping and quote escaping logic

## [0.3.0] - 2025-11-16

### 🔧 Changed - Major Architecture Improvements

#### Rust Built-in Tools MCPResult Architecture Refactoring

- **Breaking Internal Change**: Refactored Rust built-in tool servers (`content_store`, `workspace`) to return pure `MCPResult` instead of `MCPResponse`
  - Removed unnecessary JSON-RPC 2.0 transport layer generation from individual handlers
  - Centralized transport layer wrapping in Tauri Command layer
  - Eliminated 50+ duplicate `MCPResponse` creation instances across handlers
  - Unified architecture pattern with Web MCP tools
- **Trait Signature Update**: Changed `BuiltinMCPServer::call_tool()` to return `Result<MCPResult, String>` instead of `MCPResponse`
- **Request ID Management**: Moved request ID generation from individual handlers to centralized registry layer
- **Code Quality**: Reduced code complexity and improved maintainability across all built-in tools

#### Web MCP Architecture Refactoring

- **Interface Simplification**: Updated `WebMCPServer` interface to return `MCPResult` instead of `MCPResponse`
  - Built-in tools no longer handle JSON-RPC protocol details
  - Worker layer now handles all transport layer wrapping
- **Response Factory Cleanup**: Removed 38+ duplicate response factory function calls across 5 built-in servers
  - ui-tools: 6 instances removed
  - playbook-store: 14 instances removed
  - mcp-manager: 12 instances removed
  - bootstrap-server: 2 instances removed
  - planning-server: 4 instances removed
- **Consistent Error Handling**: Unified error response generation in worker proxy layer
- **Type Safety**: Improved type consistency between Web MCP and External MCP tools

### 🐛 Fixed

#### UI Resource Protocol Compliance

- **MCP-UI Protocol Fix**: Corrected UI resource structure to comply with MCP-UI specification
  - Fixed `text/html` mimeType resources to use direct `text` field instead of nested `content` object
  - Moved metadata from nested structure to `_meta` field
  - Affected tools: `export_file`, `export_zip`, `execute_shell_with_input`
- **Error Resolution**: Fixed "HTML resource requires text or blob content" error
- **Rendering Fix**: UI resources now render correctly in `UIResourceRenderer` iframe
- **Structure Validation**: All built-in UI resource generation code validated and updated

### ✨ Added

#### Tool Call Grouping UI Enhancement (from 0.2.2)

- **Collapsible Tool Groups**: Multiple consecutive tool calls now grouped into single collapsible bubble
  - Displays latest 4 tool calls by default with gradient overlay for older items
  - "Show All" button to expand and view complete execution history
- **Visual Improvements**:
  - Success/error status indicators with color coding
  - Execution time display for each tool call
  - Summary statistics (total calls, success rate, total time)
- **Smart Grouping Logic**:
  - Separates text content and UI resources into individual bubbles
  - Groups only pure tool-only calls together
  - Maintains chronological order within groups

### 📚 Documentation

- Added comprehensive refactoring documentation:
  - `docs/history/refactoring_rust_builtin_20251116_2130.md` - Rust built-in tools architecture
  - `docs/history/refactoring_20251116_2114.md` - Web MCP architecture
  - `docs/history/refactoring_20251115_1856_tool_call_grouping.md` - Tool call grouping feature

### 🔍 Technical Details

**Impact on Codebase:**

- **Files Modified**: 15+ core files across Rust and TypeScript
- **Code Removed**: ~200 lines of duplicate response wrapping code
- **Architecture**: Cleaner separation between business logic and transport layer
- **Compatibility**: External MCP servers remain unaffected (still use JSON-RPC 2.0)
- **Performance**: Slight improvement due to reduced object creation overhead

**Breaking Changes (Internal Only):**

- Internal trait signatures updated (does not affect user-facing API)
- Built-in tool handler return types changed (transparent to frontend)

### [0.2.2] - 2025-11-15

#### Added

- **Automatic Version Display**: Implemented build-time version injection system
  - Version is now automatically injected from `package.json` during build process using Vite's `define` feature
  - Added `__APP_VERSION__` global constant for consistent version display across the app
  - No more hardcoded version strings in source code
- **Version Display in UI**:
  - Added version display at the bottom of AppSidebar (visible when sidebar is expanded)
  - Added LibrAgent logo and version info in SettingsPage header

#### Changed

- **Build Configuration**: Updated `vite.config.ts` to inject version from package.json
- **Type Definitions**: Added TypeScript global type declaration for `__APP_VERSION__`
- **Linting**: Configured ESLint to recognize `__APP_VERSION__` as a global readonly variable

#### Technical Details

- Version is now maintained in a single source of truth (`package.json`)
- Build system automatically propagates version to all UI components
- Type-safe version access throughout the application

### [0.2.1] - 2025-11-15

### Patch

- Bumped version to 0.2.1 and prepared release.

### [0.3.0] - 2025-11-15

제출해주신 리팩토링 및 구현 계획 문서 전체를 분석하여 핵심 변경 사항을 요약한 Changelog를 작성했습니다.

문서들은 기능 추가, 아키텍처 재설계, 성능 최적화, 버그 수정 등 광범위한 변경을 포함하고 있습니다.

---

## LibrAgent Changelog (2025.01 ~ 2025.11 요약)

### ✨ 신규 기능 (Features)

- **AI 에이전트 기능 확장**
  - AI 서비스(`OpenAI`, `Anthropic` 등) 호출 시 `forceToolUse` 옵션을 추가하여, 모델이 반드시 도구를 사용하도록 강제하는 기능을 구현했습니다.
  - 에이전트가 런타임에 MCP 서버를 직접 관리(검색, 생성, 연결)할 수 있도록 `mcp_manager` 내장 도구를 추가했습니다. 이 도구는 BM25 기반 검색을 지원합니다.
- **비동기 프로세스 관리 (Workspace)**
  - `execute_shell` 도구에 `async` (비동기) 실행 모드를 추가했습니다. 이를 통해 서버 시작, 빌드, 파일 감시 등 장시간 실행되는 명령어를 백그라운드에서 실행하고 즉시 제어권을 반환받을 수 있습니다.
  - 비동기 프로세스를 모니터링하기 위한 도구 3종 (`poll_process`, `read_process_output`, `list_processes`)을 추가했습니다.
  - 에이전트가 `poll_process`를 과도하게(예: 5회 연속) 호출하는 패턴을 감지하고, 더 효율적인 폴링 전략(예: 대기 시간 증가)을 제안하는 가이드 시스템을 도입했습니다.
- **UI 상호작용 도구 (Agent-User)**
  - 에이전트가 사용자에게 명시적인 "계속" 확인을 요청할 수 있는 `wait_for_user_resume` 도구를 추가했습니다.
  - 에이전트가 사용자에게 텍스트 입력, 객관식 선택(`prompt_user`)을 요청하거나 데이터를 시각화(`visualize_data`)할 수 있는 `ui-tools` 모듈을 구현했습니다.
- **데이터 및 검색**
  - 메시지 저장소를 기존 프론트엔드 IndexedDB에서 백엔드 SQLite로 이전하여 데이터 관리 및 검색을 중앙화했습니다.
  - Rust 백엔드에 BM25 기반의 전역 메시지 검색 기능을 도입했습니다.
  - 'History' 페이지에 전역 메시지 검색 UI를 추가했습니다. 이제 검색 결과(히트 수)를 기반으로 관련성이 높은 세션을 우선 정렬할 수 있습니다.
- **코어 시스템**
  - Web Worker (MCP)에서 발생한 DB 변경 사항(예: `mcp_manager`로 서버 생성)을 React UI로 즉시 전파하는 실시간 이벤트 알림 시스템을 구축했습니다.
  - 단일 세션 내에서 여러 독립적인 대화 맥락을 관리할 수 있도록 `threadId` 기반의 메시지 그룹핑 기능을 도입했습니다.

---

### 🚀 아키텍처 개선 (Architecture & Refactoring)

- **Stateless 아키텍처로 전환 (핵심 변경)**
  - 기존 `switch_context`에 의존하던 stateful 아키텍처를 전면 폐기했습니다.
  - 이제 모든 도구 호출 시 `__sessionId`, `__assistantId`, `__threadId` 등 컨텍스트 정보를 파라미터로 명시적으로 전달하여, 동시 요청(concurrent request) 안정성을 확보하고 Race Condition을 원천 차단합니다.
- **데이터베이스 (Backend)**
  - Rust 백엔드의 모든 SQLx 직접 호출을 **Repository Pattern**으로 추상화했습니다. 이를 통해 데이터 접근 로직을 중앙화하고, 테스트 용이성(Mocking) 및 유지보수성을 대폭 향상시켰습니다.
- **MCP (Model Context Protocol)**
  - **설정 분리**: `Assistant` 모델이 `mcpConfig` 전체를 포함하던 구조를 리팩토링했습니다. 이제 MCP 서버는 중앙 DB에서 관리되며, Assistant는 `mcpServerIds` 배열(참조)만 갖도록 변경하여 설정 중복을 제거하고 관리를 용이하게 했습니다.
  - **공식 스펙 준수**: 기존 `stdio` 전용 MCP 설정을 HTTP/SSE Transport 및 OAuth 2.1 인증을 지원하는 공식 스펙으로 마이그레이션했습니다.
  - **메타데이터 분산**: `WebMCPServiceRegistry`에 하드코딩되어 있던 서버 메타데이터(표시 이름, 설명 등)를 각 서버 모듈이 직접 소유하도록 분산시켰습니다.
  - **도구 분리**: `playbook-store`의 `list_playbooks` (AI용 텍스트)와 `show_playbooks` (사용자용 UI) 도구를 분리하여 에이전트의 자율적 실행을 보장하고 페이지네이션을 추가했습니다.
- **AI 서비스**
  - Anthropic 서비스의 모델 목록을 정적 config 파일이 아닌, SDK API를 통해 동적으로 로드하도록 개선하여 최신 모델을 자동으로 지원합니다.
- **코드 모듈화**
  - `mcp-types.ts` (700+ 라인) 파일을 도메인별(protocol, config, schema, utils 등) 여러 모듈로 분리했습니다.
  - `playbook-store.ts`, `planning-server.ts` 등 1000라인이 넘는 대형 모듈들을 책임별 서브모듈로 분리하여 복잡도를 낮췄습니다.
- **성능 최적화**
  - 과도한 React Context 중첩 구조를 단순화하고, 스크롤 이벤트에 `throttle`, 검색 입력에 `debounce`를 적용하여 전반적인 앱 응답성을 개선했습니다.

---

### 🐛 버그 수정 (Fixes)

- **Windows 호환성 (Critical)**
  - Windows 환경에서 파일 드래그 앤 드롭 import가 실패하던 경로 파싱 문제(Unix `/` vs Windows `\`)를 해결했습니다.
  - 플랫폼별로 `execute_shell`(Unix)과 `execute_windows_cmd`(Windows)로 도구 이름을 분리하여 명령어 실행 호환성을 확보했습니다.
- **AI 서비스**
  - OpenAI API에서 `tool_calls`와 `tool` 응답 메시지의 순서가 맞지 않아 발생하던 400 에러를 해결하기 위해 Tool Call Pairing 검증 로직을 추가했습니다.

---

### 🎨 UI/UX 개선 (UI/UX)

- Agent 응답 메시지 UI를 기존의 말풍선 형태에서 ChatGPT와 같이 전체 너비를 사용하는 Rich Display 형식으로 변경했습니다.
- 긴 코드 블록이나 텍스트가 UI 영역을 벗어나던(overflow) 문제를 해결하고, 가로 스크롤바를 제공하도록 수정했습니다.
- 모든 코드 블록에 **Syntax Highlighting**을 적용하여 가독성을 높였습니다.
- Agent가 응답을 생성 중일 때, 단순 "thinking..." 텍스트 대신 애니메이션 효과가 포함된 로딩 UI를 표시하도록 개선했습니다.

## [0.2.0] - 2025-11-10

### Added

- **Interactive Shell Execution with User Prompts (Zero-Knowledge Architecture)** - [Refactoring Plan](docs/history/refactoring_20251115_1430.md)
  - Added `require_user_input` parameter to `execute_shell` tool for requesting user input before command execution
  - Implemented secure password handling via direct Tauri command (`execute_with_user_input`)
  - **Security**: User input (passwords) NEVER appears in MCP requests/responses or logs
  - **Architecture**: Frontend → Tauri IPC (encrypted) → Backend execution, bypassing MCP protocol entirely
  - Supports password (hidden) and text (visible) input types with UIResource-based prompt UI
  - Automatic sudo detection and password prompt generation
  - Pending execution state management with timeout protection (5 minutes)
  - Use cases: sudo commands, interactive scripts, confirmation prompts
  - Password is cleared from memory immediately after execution
  - See detailed implementation plan and security analysis in refactoring document

### Breaking Changes

- **Platform-specific shell execution tool names**:
  - Windows: `execute_shell` renamed to `execute_windows_cmd` to clarify cmd.exe usage
  - Unix: `execute_shell` remains unchanged (bash/sh)
  - This change improves tool naming clarity and prevents cross-platform confusion
  - Tool descriptions and examples are now platform-specific
  - **Migration**: Update any hardcoded tool name references from `execute_shell` to `execute_windows_cmd` on Windows

### Highlights

- Cross-platform build support with Windows target compilation from Linux
- Enhanced release automation via GitHub Actions
- Multi-platform binary distribution (Windows, macOS, Linux)

## [0.1.1] - 2025-10-11

### Highlights

- Improved History UX and search plumbing (global message search aggregation, session-level hit counts).
- Introduced built-in WebMCP UI tools for interactive prompts and small visualizations (prompt_user, reply_prompt, visualize_data).
- Backend message persistence improvements: groundwork for SQLite-backed message storage and Tauri commands for message CRUD.
- BM25 search integration planning and initial index metadata handling; session index cleanup implemented to remove orphaned index files.

### Added

- WebMCP UI tools (built-in): `prompt_user`, `reply_prompt`, `visualize_data`.
  - HTML-based UI resources returned as multipart MCP responses to enable interactive in-chat UI flows.
  - Worker and module registry updated to include `ui` server module.
- History search improvements:
  - `searchMessages` Tauri wrapper added to `rust-backend-client` (client-side wrappers and SWR integration).
  - Frontend aggregates message search results into session-level summaries (count, latest timestamp, snippet) for the History view.
- Session index cleanup: backend now removes BM25 index files and index metadata when sessions are deleted (reduces disk bloat).
- Sidebar & UI polish:
  - Removed legacy "Message Search (BM25)" sidebar item and unified History link to `/history`.
  - Added keyboard shortcut to toggle the sidebar (Ctrl+B).

### Changed / Improved

- Message persistence plan and tooling:
  - Added design and initial server/client commands for message pagination and upsert (Tauri commands: `messages_get_page`, `messages_upsert`, etc.).
  - Frontend `SessionHistoryContext` prepared to switch from IndexedDB optimistic persistence to backend SQLite persistence.
- BM25 search architecture documented and partially implemented:
  - Index metadata schema (`message_index_meta`) introduced to track index_path, last_indexed_at and doc_count.
  - Background reindex worker design (dirty tracking, incremental reindex) included.
- Playbook / UIResource improvements: multipart responses (text + UIResource) are used for richer tool results (playbook lists, interactive UI).

### Fixed

- Removed the unused `MessageSearch.tsx` and cleaned up related imports.
- Linting/formatting and build fixes discovered during validation.

### Notes & Next Steps

- Run the full validation pipeline before publishing: `pnpm refactor:validate` (lint, format, rust checks, build, dead-code).
- Remaining work planned for 0.1.x:
  - Finalize SQLite message persistence and production-safe migration from IndexedDB.
  - Implement `messages_search` Tauri command and BM25 index manager (load/save/search).
  - Add tests for message CRUD, search, and index persistence.
  - Consider adding a compact release note/summary to the top of `README.md`.

### Binary Checksums (SHA256)

For verifying download integrity:

- `LibrAgent_0.1.1_amd64.deb`: `44f55aeff87b755fa364119a9249486520731b041a4c980793c9dceca8efa73e`
- `LibrAgent-0.1.1-1.x86_64.rpm`: `c007bb2931a074eeb865b9dd50d3b1e4173354ea0ed57a911f7ada30fb02c00a`
- `LibrAgent_0.1.1_amd64.AppImage`: `48c0b415297d5f8bf0a0339669758842afc65c9547f9fdf3f9f0cc1121c0d853`

### Reference

- See `docs/history/*` and `docs/sprints/*` for implementation notes, design decisions, and code pointers used to prepare this release.
