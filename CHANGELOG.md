# Changelog

All notable changes to this project will be documented in this file.

## [0.7.21] - 2026-05-07

### 🚀 Features

- **Performance Optimizations**: Optimized `summarizeIpcCalls` and `AgentChatContext` by removing redundant O(N) operations, improving UI responsiveness during high-volume interactions.
- **Enhanced Chat Feedback**: Added missing LLM cancel wiring and refined `AgentChatStatusBar` with better state synchronization for more accurate progress reporting.

### 🐛 Fixes

- **[CRITICAL] Security: Path Traversal Protection**: Hardened `SecurityValidator` to prevent path traversal bypasses involving symlinks and absolute paths, ensuring safer file operations within the workspace.
- **Improved UI Stability**: Resolved a session notification race condition and fixed a UI lock issue in knowledge deletion to ensure smoother background operations.
- **LLM Reliability**: Fixed OpenAI abort signal preservation across retries, ensuring consistent cancellation behavior for streaming requests.
- **Settings Form Reliability**: Fixed advanced and performance settings form synchronization so numeric controls and saved values stay aligned instead of drifting after updates.
- **History View Resilience**: Hardened session and organization history loading, restored list semantics for assistive tech, and corrected broken troubleshooting guide links.

### 🔧 Internal

- **Test Hardening**: Relocated and refactored tests for `SecurityValidator` and added validation test cases for absolute path rejection.
- **Settings and Streaming Refactor**: Split large settings surfaces into focused sections, introduced shared numeric setting helpers, and reduced unnecessary streaming churn in agent chat internals.
- **Codebase Maintenance**: Cleaned up redundant state setters, improved internal history and knowledge performance tracking, and added model fetch policy regression coverage.

## [0.7.20] - 2026-05-05

### 🚀 Features

- **Enhanced Thinking Process UI**: Added formatted duration timers to the thinking process display and updated translation keys for better internationalization.
- **Improved Startup Experience**: Optimized the startup initialization path for faster launches and improved skill synchronization reliability during application boot.
- **UI & Palette Refinements**: Added visibility toggles for API keys with tooltip hints and improved AgentMessageBubble localization for better consistency in English and Korean.

### 🐛 Fixes

- **Message Compaction & Stability**: Fixed critical issues in message compaction, including in-flight reset paths, active request preservation, and reference preservation hints.
- **Recovery & Resilience**: Added automated recovery for non-productive completions and stream recovery for repeated thinking patterns, preventing stale response races.
- **Startup Race Conditions**: Resolved cache races and improved manifest synchronization safety to prevent intermittent startup failures.
- **Documentation & UI Polish**: Refined README files across the project for improved clarity and product positioning, and fixed minor UI blockers in skill caching.

### 🔧 Internal

- **Dependency Updates**: Bumped core backend dependencies including `tauri`, `rmcp`, `sea-orm`, and `dirs` to their latest versions.
- **Message Source Unification**: Unified message source creation and centralized source semantics across the Rust-TypeScript boundary for more robust tracking.
- **Cleanup & Maintenance**: Standardized compaction summary wrappers and cleaned up stale assistant artifacts and temp files.

## [0.7.19] - 2026-05-04

### 🚀 Features

- **Settings & Localization Expansion**: Expanded Rosetta coverage for settings and improved Korean (KO) translations for better consistency.
- **Enhanced Agent UI Tooltips**: Added helpful tooltips to skill deletion actions and icons in the settings and palette views to improve discoverability.

### 🐛 Fixes

- **Agent Chat UX & Scroll Stability**: Fixed critical chat scroll anchoring and pinning issues, especially during streaming and session transitions, ensuring the view stays locked to the latest content.
- **Flicker-Free Session Switching**: Reduced UI flickering and improved transition smoothness when switching between different agent sessions.
- **Reliable MCP Preset Management**: Resolved gaps in MCP preset registry loading and install state tracking for more predictable extension management.
- **Workspace Search Precision**: Fixed artifact filtering in workspace searches to prevent internal agent files from cluttering search results.

### 🔧 Internal

- **Refined Event Handling**: Improved session event handling and state synchronization across agent chat and session routes.
- **Test Suite Hardening**: Expanded and stabilized test coverage for agent chat, session management, and workspace search logic.

## [0.7.18] - 2026-05-02

### 🚀 Features

- **Teamwork Contract Hardening**: Significantly hardened the agent teamwork workspace contract and artifact filtering logic for more reliable multi-agent collaboration.
- **Artifact Isolation**: Isolated internal artifacts to prevent workspace pollution and improved the canonical filtering of agent-generated files.

### 🐛 Fixes

- **Path Normalization**: Fixed path normalization for workspace export and download operations to ensure cross-platform path consistency.
- **Git Worktree Safeguard**: Added safeguards to refuse git worktrees before scaffold directory creation, preventing potential state corruption.

### 🔧 Internal

- **Test & Service Optimization**: Synchronized builtin services output and skipped runtime state sequence tests on Windows to improve CI stability.

## [0.7.17] - 2026-05-01

### 🚀 Features

- **Rosetta Localization Polish**: Localized settings placeholders and expanded Korean (KO) translation coverage for a more consistent international experience.
- **Draft Input & Indexing Refinement**: Improved draft input reuse and refined file indexing logic to enhance the reliability of workspace-aware drafting.

### 🐛 Fixes

- **Runtime Stability & Hardening**: Hardened runtime state sequencing and implemented panic handling for proxy discovery tasks to ensure a steadier application lifecycle.
- **Draft Chat UI Alignment**: Aligned the draft chat frame with the active chat UI for better visual consistency across session transitions.

### 🔧 Internal

- **Session State & Proxy Optimization**: Unified session runtime state management and optimized proxy initialization for faster and more reliable session startup.
- **Bolt Performance Tweak**: Removed redundant array reductions in Bolt's internal logic to slightly improve data processing efficiency.

## [0.7.16] - 2026-04-29

### 🚀 Features

- **Optimized Startup Performance**: Implemented deferred loading for the startup assistant and registry, added startup instrumentation, and split the agent startup route to reduce initial load time and prevent UI blockers.
- **Enhanced Agent Chat UX**: Fixed scroll anchoring issues and polished the chat composer interaction for a smoother messaging experience.
- **Localization Expansion**: Expanded Rosetta coverage for DangerZone settings and improved Korean (KO) translations for better consistency.
- **A11y & Semantic Polish**: Converted `PlaybookGroup` container to a semantic button to improve keyboard navigation and screen reader accessibility.

### 🐛 Fixes

- **Reliable Workspace Hydration**: Fixed edge cases where workspace overrides would fail to hydrate during idle sessions or trigger flaky test results.
- **Startup Registry Blockers**: Resolved race conditions in registry loading that could occasionally hang the application during the boot sequence.

### 🔧 Internal

- **Performance Optimizations**: Memoized `completedTodos` calculations in planning panels and refactored `useScheduledTasks` to use a declarative SWR pattern for more efficient state management.
- **React Render Refactor**: Applied the "Adjusting State During Render" pattern to `ToolCallCompactItem`, eliminating redundant `useEffect` synchronizations and improving UI responsiveness.
- **Maintenance & Cleanup**: Removed the JS bundle budget gate and applied consistent code style formatting throughout the codebase.

## [0.7.15] - 2026-04-28

### 🚀 Features

- **Broader Accessibility and Localization Polish**: Added clearer ARIA labels in chat/settings surfaces and expanded localized settings copy so everyday navigation feels more consistent and understandable.

### 🐛 Fixes

- **Knowledge Browser Pagination Stays Consistent**: Prevented stale "load more" responses from leaking old pages into newly filtered or refreshed knowledge results.
- **Agent Chat Bottom-Follow Feels More Accurate**: Adjusted bottom-threshold handling around the floating input area so new activity stays anchored more reliably near the visible bottom of the chat.
- **Safer Workspace Line Edits**: Tightened `editFiles` validation and guidance so invalid line-edit inputs are rejected earlier and workspace anchor behavior stays more predictable.

### 🔧 Internal

- **Knowledge Browser Refactor**: Split the knowledge browser state and detail dialog into smaller hooks and components, reducing maintenance drag without changing the overall workflow.
- **Session Metadata and Dependency Maintenance**: Cached repeated session metadata access and refreshed supporting Rust dependencies and generated assets to keep the release branch tidy.

## [0.7.14] - 2026-04-27

### 🚀 Features

- **Broader UI Polish Across Agent Workflows**: Expanded localization coverage and keyboard/focus improvements across agent, playbook, settings, and MCP server surfaces to make day-to-day navigation feel steadier and more accessible.

### 🐛 Fixes

- **Paused Session Model Switching**: Restored the ability to change the active model/provider while an agent session is paused, keeping recovery and resume workflows flexible instead of forcing a full stop.
- **Leaner File Mentions and Safer Draft Attachments**: Stopped `@file:` mentions from inlining whole file bodies into prompt context and fixed draft attachment preprocessing so file-to-attachment mapping stays correct even when some files fail earlier preprocessing steps.
- **More Reliable Chat, Compaction, and Shell Behavior**: Tightened agent chat rendering/layout behavior, aligned compaction request/layout handling, and hardened shell/process output behavior so long-running sessions and workspace tooling stay more predictable.

### 🔧 Internal

- **Large-Scale Codebase Cleanup**: Split oversized frontend providers/pages and modularized the logger and Ollama service internals, reducing maintenance drag without changing the public app surface.
- **Release and Diagnostics Maintenance**: Improved nightly release workflow portability and expanded architecture/refactor documentation to support safer ongoing development.

## [0.7.13] - 2026-04-25

### 🚀 Features

- **Enhanced History Deletion**: Added delete capability to `OrgCard` in the history view, allowing users to remove organization sessions directly.

### 🐛 Fixes

- **Session Loading & Readiness**: Hardened agent session loading and initialization readiness to prevent race conditions during startup.
- **Compaction Lifecycle Improvements**: Refined the compaction lifecycle and threshold handling to reduce database pressure and improve long-term session stability.
- **Localization Expansion**: Localized GeneralTab font settings and expanded Korean (KO) translation coverage via the Rosetta system.
- **UI & Formatting Polish**: Standardized formatting for the general settings tab and fixed agent message locale keys for a more consistent interface.

### 🔧 Internal

- **Performance Optimizations**: Optimized knowledge vector bulk delete and preflight token estimation for faster background operations.
- **Error Handling**: Improved graceful handling of compaction finish emit failures and restored terminal LLM error propagation.

## [0.7.12] - 2026-04-24

### 🚀 Features

- **Markdown Tables in MCP History**: Optimized MCP history responses using Markdown tables for better readability and structured information presentation.
- **Enhanced Token Dropdown**: Refactored `InputTokenDropdown` using a Custom Hook Pattern, improving maintainability and the navigation experience.

### 🐛 Fixes

- **LLM Recovery & Stability**: Hardened preflight overflow recovery and stale response handling to ensure smoother agent operations under context pressure.
- **List Navigation Polish**: Resolved edge cases in list navigation to provide a more consistent keyboard and focus experience.
- **Rust Backend Formatting**: Fixed formatting inconsistencies across the Rust codebase to ensure consistency with project standards.

### 🔧 Internal

- **Windows Test Stability**: Stabilized date utility tests specifically for Windows environments to ensure reliable CI pipelines.
- **Performance Optimization**: Applied `React.memo` to `ScheduledTaskRow` to prevent inefficient O(N) list re-renders, improving responsiveness in large task lists.

## [0.7.11] - 2026-04-22

### 🚀 Features

- **Unified Browser Content Reading**: Consolidated browser content retrieval around a clearer `getPageContent` flow, making it easier for agents to read current page state without juggling overlapping content tools.

### 🐛 Fixes

- **Safer Delegated Session Control**: Locked delegated agent controls (`checkSession`, `messageToSession`, `stopSession`, and manual compaction) to real descendant sessions only, preventing cross-lineage access and improving caller-session validation.
- **Stronger MCP Server Registration Guards**: Fixed duplicate MCP server registration so lookup failures no longer fall through into accidental config mutation, and duplicate names are rejected more reliably.
- **More Reliable Browser Cache Behavior**: Tightened browser session cache invalidation so content reads stay aligned with the active session state instead of serving stale page data.

### 🔧 Internal

- **Richer Tool Inventory Contracts**: Added structured payloads to tool-list responses and expanded regression coverage around delegated-session access, browser guidance contracts, and MCP server registration behavior.

## [0.7.10] - 2026-04-21

### 🚀 Features

- **Expanded Localization**: Enhanced Korean translation coverage for the Settings page and unified translation patterns across the Rosetta system.

### 🐛 Fixes

- **MCP State Preservation**: Fixed an issue where unique IDs and creation dates were lost during MCP server updates, ensuring data continuity and reliable state management.
- **Preset Restoration**: Restored missing HTTP presets for GitHub and Exa, and fixed a broken logo reference in the HuggingFace MCP preset.

### 🔧 Internal

- **Workflow Cleanup**: Removed the obsolete `fre4x-inspector-bridge` and updated the refactor validation workflow to improve development efficiency.
- **Data Integrity**: Added unit tests for MCP server update logic to prevent regressions in data preservation.

## [0.7.9] - 2026-04-19

### 🚀 Features

- **Global Knowledge Manager UI**: Introduced a new dedicated page for managing global knowledge, including improved repository handlers and unified backend commands for a more streamlined information management experience.
- **Refactored Skills Management**: Overhauled the skills management panel with enhanced conflict resolution, reset dialogs, and a custom hook-based architecture, significantly improving the reliability of skill lifecycle operations.

### 🐛 Fixes

- **Robust Workspace Edits**: Prevented duplicate start-of-file edits and hardened the robustness of edit action tagging in workspace tools to ensure precise and reliable file modifications.
- **Sub-session LLM Inheritance**: Fixed an issue where sub-sessions would occasionally fail to inherit the correct LLM configuration from their parent session.
- **Circuit Breaker Recovery**: Refined circuit breaker recovery handling to ensure smoother and more predictable transitions back to healthy states after transient service failures.

### 🔧 Internal

- **Workspace Edit Line Refactor**: Reorganized the workspace `edit_line` module into a cleaner, modular structure for better maintainability and extensibility.
- **Tool Description Cleanup**: Refined tool descriptions by removing redundant default values and internal implementation details, reducing prompt bloat and improving agent comprehension.
- **Large File Finder Improvements**: Updated the `find_large_files.sh` script for better performance and accuracy when identifying oversized assets.

## [0.7.8] - 2026-04-18

### 🚀 Features

- **Intelligent Loop Prevention**: Implemented a sophisticated loop prevention system with configurable thresholds and natural recovery mechanisms to improve agent stability.
- **Unified Workspace File Operations**: Promoted and unified the `editFiles` contract, enabling robust multi-file operations and improved backward compatibility.
- **Enhanced Dropdown Navigation**: Added automatic scroll-into-view for active items in token dropdowns, improving the keyboard navigation experience.

### 🐛 Fixes

- **Robust macOS Releases**: Upgraded CI release pipeline with hardened macOS signing and notarization validation to ensure reliable distribution.
- **Refined Skill Management**: Overhauled skill installation flows and updated documentation examples for better clarity and reliability.
- **Improved Error Handling**: Normalized LLM billing limit errors and corrected agent playbook execution flows.
- **UI Stability**: Resolved memoization issues in settings and fixed various typing and rendering inconsistencies across the interface.

### 🔧 Internal

- **Performance Optimizations**: Optimized internal metric calculations and reduced database connection overhead in the test suite.
- **Architecture Refactoring**: Modularized the agent message renderer and skill management logic for better maintainability.
- **Best Practices Update**: Updated documentation and rules for React best practices to align with latest performance patterns.

## [0.7.7] - 2026-04-16

### 🚀 Features

- **Overhauled Skills Management**: Significantly improved skill management, including hardening of managed skill installations and better synchronization of legacy global skills with bundled snapshots.
- **Enhanced Workspace UI**: Added informative tooltips to workspace panel actions and improved accessibility/labeling for a more intuitive navigation experience.

### 🐛 Fixes

- **Robust Skill Parsing and Syncing**: Fixed skill frontmatter BOM parsing issues and ensured skills resource synchronization remains reliable across managed and bundled skills.
- **Improved Workspace Reliability**: Resolved directory listing path handling on Windows and allowed '..' in directory names for more flexible file system navigation.
- **Agent and Tool Stability**: Corrected external tool counting and resolved build script IO errors to ensure smoother agent and tool operations.

### 🔧 Internal

- **Performance Optimization**: Refined Bolt's internal logic by replacing `.filter().length` with `.reduce()` for better efficiency in data processing.
- **Code Maintenance**: Cleaned up session imports in `AgentChatContext` and refined build script error handling.

## [0.7.6] - 2026-04-14

### 🐛 Fixes

- **Recovered Sessions and Scheduled Tasks Are Steadier**: Fixed proxy rehydration/readiness races and config fallback issues so resumed sessions and scheduled tasks no longer trip over missing external MCP managers on their first tool call.
- **Knowledge Search Handles Punctuation Again**: Sanitized Knowledge v2 full-text queries so searches containing ampersands, parentheses, and other special characters stop breaking SQLite keyword matching.
- **Safer Interactive HTML Rendering**: Hardened `presentInteractive` HTML sanitization with a strict allowlist while preserving basic tables and links, reducing unsafe markup exposure without turning simple reports into garbage.
- **Cleaner Agent and Assistant Flows**: Tightened agent list pagination, trimmed noisy tool/service context output, and refined assistant validation so browsing and configuring agents feels more reliable.

### 🔧 Internal

- **Native Teamwork Skill Cleanup**: Renamed bundled native teamwork skills to shorter names (`teamwork`, `org`, `schedule`, `delegate`) and aligned the related scaffolding and regression coverage.
- **Editor and Scheduling Maintenance**: Refactored `SkillsEditor` drag-and-drop internals and simplified scheduled-task next-run calculation logic to reduce UI maintenance overhead.

## [0.7.5] - 2026-04-12

### 🚀 Features

- **Reorganized Settings Layout**: Moved UI visuals (Font, Metrics, Tool Details) to the General tab and File/Workspace settings to the System tab for a more intuitive configuration experience.
- **AI & Models UX Polish**: Reordered the AI & Models tab to prioritize model selection at the top, allowing faster primary and fallback LLM adjustments.

### 🐛 Fixes

- **Provider Config and Model Picker Sync**: Fixed an issue where model pickers would fail to refresh immediately after saving new provider API keys or base URLs.
- **Ollama and Attachments Reliability**: Improved Ollama model listing stability and session-attachment handling for smoother local model workflows.
- **Compaction Flow Hardening**: Strengthened session compaction logic and UI boundary detection to prevent display drifts and inconsistent states during long conversations.

### 🔧 Internal

- **Implicit Gemini Caching**: Switched the Gemini service to use implicit model caching, streamlining request assembly and improving cache hit reliability.
- **Session Compaction Tooling**: Introduced new internal tools and anchors for manual session compaction to support advanced context management.
- **Code Quality and Cleanup**: Standardized Rust compaction file formatting and removed redundant handlers and props from the Settings feature to improve maintainability.

## [0.7.4] - 2026-04-10

### 🐛 Fixes

- **More Reliable Long-Context Recovery**: Tightened compaction preflight and context-budget handling so long-running sessions recover more cleanly when they hit context pressure instead of drifting into inconsistent retry states.
- **Gemini Cache Alignment**: Refined Gemini request prefix caching and related preflight behavior so cache reuse stays more stable across repeated requests.
- **Windows CI Stability**: Fixed a Windows-only Rust integration test crash in compact-recovery coverage, keeping the release pipeline consistent across platforms.

### 🔧 Internal

- **Large File Read Guidance Cleanup**: Improved chunking guidance and supporting tests around large workspace reads so agent/tool behavior is easier to reason about and validate.

## [0.7.3] - 2026-04-10

### 🚀 Features

- **Draft Workspace Preview and File Mentions**: Added a draft workspace preview surface with smarter file-tree shaping and mention handling so agents can inspect and reference staged workspace files more comfortably before sending.

### 🐛 Fixes

- **Compaction Retry Recovery**: Fixed a same-tail retry edge case in context compaction so long-running sessions can retrigger compaction instead of getting stuck past the configured limit.
- **Provider Tool-Result Prompt Inflation**: Stopped structured tool metadata and JSON-looking tool text from being promoted into provider prompt payloads, fixing Gemini prompt bloat and aligning tool-result handling across providers.

### 🔧 Internal

- **Targeted Diagnostics and Regression Coverage**: Added focused logs and regression tests around compaction telemetry, Gemini usage accounting, and provider tool-result conversion to make future prompt-drift issues easier to catch.

## [0.7.2] - 2026-04-09

### 🚀 Features

- **Settings Workflow Overhaul**: Reorganized Settings into clearer General / AI Models / Chat / System / Advanced areas, added sticky tab state, discard support, unsaved-change guarding, and stronger destructive-action handling so configuration changes feel safer and less chaotic.
- **Smarter Model and Tool Guidance**: Improved Gemini prompt-cache handling, expanded browser tool guidance around cached pagination, and refined tool inventory browsing so agents get clearer instructions when reading content or discovering tools.

### 🐛 Fixes

- **Settings That Actually Match Reality**: Removed misleading or unused settings such as the session workspace capacity cap, wired retry-related model settings into live runtime behavior, and clarified restart-required system settings inline.
- **Assistant Search and Tool Inventory Reliability**: Moved assistant searching into the Rust backend, fixed stale search clearing behavior, and corrected tool-list pagination/counting so empty external servers no longer skew results.
- **Accessibility and Agent UI Polish**: Improved disabled-button tooltip behavior, status-bar recovery, and keyboard-focus labeling across agent chat controls so the interface is steadier and more accessible.

### 🔧 Internal

- **Search, Settings, and Agent Refactors**: Split large search and Gemini service logic into cleaner modules, continued cleanup across planning-update event handling, and added focused regression coverage for settings danger-zone confirmation and related UI behavior.

## [0.7.1] - 2026-04-08

### 🚀 Features

- **Safer and Clearer Agent Message UI**: Added stronger keyboard-focus treatment for image actions and improved link handling so message content feels more polished and trustworthy during navigation.
- **Effective Context Visibility**: Clarified the agent context meter to reflect Rust's effective request-time context calculation, making compaction-aware usage easier to understand during long sessions.

### 🐛 Fixes

- **Security: Hardened External Link Sanitization**: Blocked unsafe and scheme-relative URLs in markdown and resource links, closing XSS-style fallback paths in rendered agent content.
- **Compact Mode Contract Alignment**: Fixed compact-mode context accounting and selector behavior so frontend and Rust stay aligned on what counts toward the real request budget.

### 🔧 Internal

- **Workspace Tool Response Cleanup**: Refined workspace directory/search pagination and response shaping to keep large tool outputs more efficient and easier for agents to consume.
- **Release and UI Maintenance**: Refreshed supporting UI primitives, styling details, and compaction documentation to match the current product behavior more accurately.

## [0.7.0] - 2026-04-06

### 🚀 Features

- **Team Workspace, Org, and Scheduling Expansion**: Added the new teamwork/org session model with richer org history views, chart-backed lineage summaries, and scheduled-task grouping/governance so multi-agent collaboration is easier to organize and inspect.
- **Sharper Agent and Workspace Tooling**: Refined builtin agent, planning, browser, media, UI, and workspace tool flows, including split workspace editing surfaces and improved tool guidance for more capable agent execution.
- **Settings and Form UX Polish**: Smoothed out settings responsiveness and refreshed shared input/label patterns so forms and configuration screens feel faster and more consistent.

### 🐛 Fixes

- **Planning Todo Reliability**: Corrected planning `todoId` schema behavior and related workflow handling so agents are less likely to target invalid todos or stall on planning actions.
- **Agent Workflow Stability**: Fixed assorted regressions around agent event handling, status updates, and workspace/tool interactions to keep long-running sessions and scheduled-task flows steadier.

### 🔧 Internal

- **Architecture Refactor for Teamwork**: Landed the large teamwork refactor across session metadata, repositories, MCP builtin services, and scheduled-task plumbing to support the new org-centric model cleanly.
- **Regression Coverage and Developer Tooling**: Expanded Rust/frontend test coverage around org context, tool discovery, workspace anchors, scheduling policies, and LLM event flows, while adding bundled skill and documentation updates.
- **Dependency and Maintenance Updates**: Refreshed supporting Rust/Tauri dependencies and continued cleanup across AI service contracts, release tooling, and workspace internals.

## [0.6.31] - 2026-04-04

### 🚀 Features

- **Enhanced Agent UI & Feedback**: Added tooltips to icon-only buttons across the Palette and improved planning toast notifications to prioritize recent activity for better visibility.
- **Improved Performance & Reliability**: Optimized prompt caching and streaming feedback, and hardened the AI service compaction flow for more stable long-running sessions.
- **UX Polish**: Added tooltips to bookmark buttons in PlaybookCard and standardized code formatting and indentation across multiple components.

### 🐛 Fixes

- **Stability & Recovery**: Fixed recovery pause and renderer handling issues to prevent workflow stalls during agent execution.
- **Settings & UI Consistency**: Resolved an issue in settings where the skills directory input would snap back, and addressed PR review feedback for better code quality.
- **Accessibility & UI Hierarchy**: Removed redundant and no-op `TooltipProvider` wrappers to clean up the component tree and improve UI consistency.

### 🔧 Internal

- **Architectural Refinement**: Modularized AI service base contracts and refactored agent session manager helpers to improve maintainability and separation of concerns.
- **Technical Debt Reduction**: Eradicated redundant `useEffect` hooks and prop hoarding in UI components (e.g., `useSkillsDirectory`).
- **Documentation & Skills**: Included updated architecture documentation and the new `code-audit-expert` skill.

## [0.6.30] - 2026-04-03

### 🚀 Features

- **Expanded Agent Session Controls**: Added new agent session management utilities, backend handlers, and UI plumbing so session lifecycle actions and status handling are more capable and consistent across the app.

### 🐛 Fixes

- **Agent Workflow Stability**: Fixed staged session transitions, suspend-permit rollback, and ToolCallCompactItem expansion behavior to prevent workflow stalls and UI regressions during agent execution.
- **Accessibility & UI Polish**: Directory child-count badges in the workspace file tree now appear on keyboard focus, improving parity between mouse and keyboard navigation.
- **Security: Database Backup Hardening**: Closed a `VACUUM INTO` SQL injection edge case in the database backup flow.
- **Date Formatting Cache Reliability**: Centralized `Intl.DateTimeFormat` caching and made formatter cache keys deterministic so localized date rendering stays efficient without leaking redundant formatter instances.

### 🔧 Internal

- **React Render Cleanup**: Removed redundant render-time work across agent UI components and hooks, including sidebar sorting and state-sync anti-pattern cleanup.
- **Reliability & Regression Coverage**: Added focused Rust and frontend regression coverage around concurrency gates, compaction recovery, and date utility behavior.

## [0.6.29] - 2026-04-01

### 🐛 Fixes

- **Database Startup Recovery for Existing Users**: Restored the previously shipped `m20260327_000025_add_stores_session_index` migration as a compatibility no-op so upgraded apps no longer fail to boot when older databases already record that migration version.

### 🔧 Internal

- **Migration Compatibility Guardrail**: Added focused regression coverage to ensure shipped migration versions remain registered in the migrator and cannot be silently dropped again.

## [0.6.28] - 2026-04-01

### 🐛 Fixes

- **Tool Discovery Availability Guidance**: Refined builtin and MCP tool discovery so agents can browse full tool inventory by default while switching to session-aware availability checks only when needed, reducing misleading "not available" guidance during planning and delegation workflows.

### 🔧 Internal

- **Tool Discovery Regression Coverage**: Added focused integration coverage to lock in canonical `agent__update` guidance and the new inventory-versus-session discovery behavior.

## [0.6.27] - 2026-04-02

### 🚀 Features

- **Builtin Services Codegen**: Automated generation of constants and types from Rust definitions to ensure perfect frontend-backend alignment and type safety.
- **Enhanced Agent UI & Multilingual Support**: Introduced the new `AgentDraftChatView` and expanded multi-language support (Spanish, French, Japanese, Chinese, Korean) for a more international experience.
- **Interactive Status & Monitoring**: Added a real-time agent chat status bar and session management utilities to improve the visibility of background tasks and session lifecycle.
- **UX Responsiveness**: Improved UI snappiness in the `AssistantEditor` and `WorkspacePanel` with loading indicators and optimized rendering patterns.
- **Secure Media Downloads**: Switched to native platform save dialogs for media downloads to improve consistency and user control.
- **Provider Hardening**: Enhanced Gemini prompt caching and handle management for more reliable long-running agent sessions.

### 🐛 Fixes

- **Security: SQL Injection Protection**: Hardened database backup logic with stricter validation to prevent potential injection vulnerabilities in the backup pipeline.
- **LLM State Consistency**: Fixed Gemini prompt cache stack handling and restored missing tabs/sub-editors in the Assistant Editor.
- **Accessibility & UX Polish**: Added ARIA labels to icon-only buttons and resolved test timeouts to ensure a more stable developer and user experience.

### 🔧 Internal

- **Architectural Cleanup**: Eliminated redundant state syncs in `SessionHistoryPanel` and optimized session sorting for better performance.
- **Reliability & Testing**: Tightened stdio isolation design tests and expanded regression coverage for PR comment reviews.
- **Database Optimization**: Added performance indexes to the `stores` table for faster session and store lookups.

## [0.6.26] - 2026-03-30

### 🚀 Features

- **Richer Agent Planning Visibility**: The planning sidebar now surfaces scratchpad notes directly, while closed-panel planning updates appear as Sonner toasts with checklist summaries and a live task progress bar.
- **Refined Agent Workspace & Planning Panels**: Refreshed the agent side panels and session header with a cleaner, more polished visual treatment so the chat workspace feels more like a finished product than a developer-only surface.
- **Polished Tools Inventory Modal**: Redesigned the agent tools modal to match the upgraded panel language with cleaner hierarchy, calmer metadata treatment, and a more readable schema inspection surface.

### 🐛 Fixes

- **ID Schema Hardening for Planning and Scratchpad**: Removed misleading numeric bounds from `todoId` and scratchpad note ID schemas so agents are no longer nudged toward bogus identifier assumptions, while keeping backend validation responsible for real existence checks.
- **Font Setting Now Reaches Agent Chat Surfaces**: Fixed the agent chat and draft chat containers so user-selected UI fonts are no longer accidentally overridden by blanket monospace wrappers.
- **Scrollable Planning and Tools Surfaces**: Fixed overflow handling in the planning panel and tools modal so long todo lists, scratchpad notes, and large tool inventories remain fully accessible instead of clipping at the bottom.

### 🔧 Internal

- **Planning State Parsing & Regression Coverage**: Added typed parsing for planning/scratchpad service contexts and expanded focused regression tests for scratchpad schema behavior and planning state handling.
- **Naming Cleanup for Session UI**: Renamed the old terminal-themed session header component to match its real purpose and removed stale shell-flavored UI references.

## [0.6.25] - 2026-03-30

### 🐛 Fixes

- **Planning Todo Schema Bounds**: Fixed the `updateTodo` tool schema so `todoId` is constrained with `minimum: 1` instead of the broken `maximum: 1`, preventing models from being steered toward invalid `todoId=1` calls.

### 🔧 Internal

- **Planning Schema Regression Coverage**: Added focused schema assertions to ensure future planning tool changes keep `todoId` validation aligned with the backend's real identifier behavior.

## [0.6.24] - 2026-03-30

### 🐛 Fixes

- **Planning Todo Targeting**: Reworked `updateTodo` to use stable `todoId` values instead of fragile list indexes, eliminating cases where agents could mark or cancel the wrong task when the visible planning context omitted some items.

### 🔧 Internal

- **Planning Guidance Alignment**: Updated planning tool schemas, service-context output, guidance text, and regression coverage so todo identifiers stay consistent across prompts, tool calls, and backend resolution.

## [0.6.23] - 2026-03-30

### 🚀 Features

- **Claude Channel Session Plumbing**: Added end-to-end channel routing and permission relay support so Claude-style channel notifications can be injected into the active agent workflow more reliably.
- **Smarter Media Materialization**: Improved session message preprocessing so the latest inline or file-backed media stays available to multimodal models without bloating older message history.

### 🐛 Fixes

- **Channel Message Hardening**: Fixed message-source deserialization and hardened injected channel payload formatting so unsafe metadata keys no longer produce malformed channel tags.
- **Workspace Scope and Prompt Cache Stability**: Corrected workspace-local file handling and preserved compaction prompt-cache layout to reduce context mismatches during long-running agent sessions.
- **Agent Session UI Polish**: Cleaned up several status, attention, and workflow rendering edge cases across the agent chat and scheduled-task surfaces.

### 🔧 Internal

- **Backend and Logging Cleanup**: Refined launch log-level overrides, continued the agent session backend refactor, and normalized supporting type exports and formatting across touched modules.
- **Docs and Regression Coverage**: Updated Tauri/channel documentation and added focused Rust and Vitest regression coverage for channel routing, workspace file scope, compaction recovery, and media preprocessing.

## [0.6.22] - 2026-03-29

### 🐛 Fixes

- **Compaction Failure Recovery**: Fixed a workflow hang edge case so preflight compaction failures now transition agent sessions cleanly into an error state with proper retry-safe recovery instead of leaving the UI stuck loading.
- **Workspace Search Accuracy**: Improved workspace search so skip reporting stays accurate and nested `.gitignore` rules inside the workspace are now respected during recursive searches.
- **Workspace File Guidance**: Cleaned up duplicate-file write guidance so numbered recovery steps render correctly and point agents toward the right overwrite, append, read, and edit actions.

### 🔧 Internal

- **Prompt Cache Stability**: Further stabilized provider cache behavior by tightening prompt diagnostics, normalizing edge-case serialization for cache fingerprints, and reducing noisy OpenAI cache logging to debug level.
- **Prompt Context Ordering**: Added volatility-aware service-context ordering so stable prompt sections stay grouped ahead of live state, improving prompt consistency for long-running sessions.
- **Regression Coverage Expansion**: Added focused Rust and Vitest coverage for compaction recovery, workspace guidance formatting, nested `.gitignore` handling, and prompt-cache helpers.

## [0.6.21] - 2026-03-28

### 🚀 Features

- **Builtin Session History Tools**: Added new tools (`listHistory`, `searchHistory`) allowing agents to discover and search through past sessions directly within the chat context.
- **Knowledge Server v2 Stabilization**: Completed graph persistence and stabilized the Knowledge v2 server, enabling robust entity and relationship extraction.
- **Cascade Session Deletion**: Implemented recursive deletion of session trees in both the Rust backend and frontend, ensuring clean cleanup of related agent sessions.
- **UI Resource Height Capping**: Limited the maximum height of interactive UI resources to `80vh` to prevent infinite growth loops and improve visual stability during long interactions.

### 🐛 Fixes

- **MCP Error Semantics Alignment**: Refactored guided error responses to strictly align with MCP semantics, improving compatibility and agent-facing guidance.
- **Workspace Search Hardening**: Hardened workspace search implementations and fixed edge cases in compaction normalization for more reliable context management.
- **Streaming Tool Feedback**: Improved real-time feedback and status reporting for streaming tool calls, reducing UI flickering and improving perceived latency.

### 🔧 Internal

- **ToolCall Rendering Optimization**: Refactored `ToolCallCompactItem` using the "Adjusting State During Render" pattern to eliminate redundant `useEffect` calls and improve rendering performance.
- **Prompt Cache Stability**: Enhanced prompt cache hit detection and metrics reporting for OpenAI, Anthropic, and Gemini providers.
- **Date Formatting Performance**: Optimized `Intl.DateTimeFormat` usage by implementing a caching layer in `date-utils`.
- **Documentation Polish**: Clarified Rust/Tauri setup instructions in `CONTRIBUTING.md` and updated architecture references in `README.md`.

## [0.6.20] - 2026-03-26

### 🔧 Internal

- **CI Recovery**: Fixed release pipeline failure by ensuring proper changelog section alignment.

## [0.6.19] - 2026-03-26

### 🐛 Fixes

- **D2Coding Font Loading**: Fixed a malformed CDN URL that prevented the D2Coding font from loading correctly.
- **Attachments Migration**: Completed the naming migration in test files (`test_migration.rs` and `test_recent_uploads.rs`) to match the new `AttachmentsServer` and `AttachmentsStorage` types.

### 🔧 Internal

- **Test Suite Modernization**: Updated the attachment test suite to use the new naming conventions and verified all tests pass on the latest schema.

## [0.6.18] - 2026-03-26

### 🚀 Features

- **UI Font Selection**: Added a user-selectable UI font setting (Inter, Nanum Square Neo, D2Coding, etc.) in the Chat Interface settings, allowing users to customize the global application font.
- **Bundled Agent Skills**: Introduced `crew-constructor` and `specialist-creator` bundled skills to streamline the creation of specialized AI agents and multi-agent teams.
- **Improved Anthropic Integration**: Enhanced Anthropic message conversion, tool-input parsing, and caching support for better reliability and performance with Claude models.
- **Optimized File Drop UX**: Implemented parallel processing and optimistic UI updates for file uploads, significantly improving the responsiveness of the agent chat interface.

### 🐛 Fixes

- **Attachments Migration**: Completed the transition from legacy `content_store` naming to the canonical `attachments` convention across the frontend, backend, and database schema.
- **Tool Naming Standardization**: Updated workspace tool references to use the new `tool__list` and `editFile` naming conventions for consistency.

### 🔧 Internal

- **Built-in Service Synchronization**: Enhanced the `sync-builtin-services.cjs` script to automatically generate canonical, core, and optional service alias exports.

## [0.6.17] - 2026-03-24

### 🚀 Features

- **SWR-based Data Fetching**: Refactored agent and server tool fetching hooks to use the `useSWR` pattern, improving caching, reactivity, and eliminating redundant imperative logic.
- **Oversized Tool Result Handling**: Implemented a "spillover" mechanism that automatically redirects oversized tool outputs to the workspace when they exceed context limits, preventing message bloat while keeping results accessible.
- **Intl.NumberFormat Caching**: Optimized rendering performance by caching `Intl.NumberFormat` instances, avoiding expensive constructor calls during high-frequency UI updates.

### 🐛 Fixes

- **Empty Response Handling**: Hardened the preflight and response parsing logic to gracefully handle empty LLM responses and compact preflight states.
- **Playbook Date Formatting**: Fixed a formatting regression in playbook card date displays to ensure consistent locale-aware presentation.

### 🔧 Internal

- **Enhanced Test Coverage**: Added regression tests for tool result spillover, cached number formatters, and CI stability.
- **Documentation Polish**: Cleaned up build instructions in `CONTRIBUTING.md` to reflect current project standards.

## [0.6.16] - 2026-03-23

### 🚀 Features

- **Smarter Agent Skill Bootstrapping**: Expanded the bundled skill stack around persona awakening and session attachments, making it easier for agents to discover their identity files and work with uploaded content through the newer streamlined attachment tool flow.

### 🐛 Fixes

- **Attachment Tool Reliability**: Restored internal attachment write routing and corrected AI-facing attachment guidance so agents use the right session-scoped file tools instead of malformed calls.
- **Session Attention Consistency**: Fixed an edge case in viewed-state handling where attention indicators could survive even after a session had effectively already been viewed past the attention timestamp.
- **Computer Diagnosis Skill Stability**: Repaired the bundled `computer-diagnosis` skill documentation and hardened system info collection so environments without CPU frequency reporting no longer break the diagnostic workflow.

### 🔧 Internal

- **UI and Platform Polish**: Replaced lingering native title tooltips with proper accessible tooltip components, cached `Intl` formatter instances for smoother rendering, and refreshed supporting dependency / workflow plumbing for the release pipeline.
- **Documentation Reorganization**: Split and reorganized architecture documentation so the growing docs set is easier to navigate and maintain.

## [0.6.15] - 2026-03-22

### 🚀 Features

- **Expanded Bundled Skill Library**: Added the new `playbook-creator`, `computer-diagnosis`, and `deep-research-report` bundled skills, along with updated supporting references and templates, so agents ship with stronger guided workflows for structured planning and investigation tasks.

### 🐛 Fixes

- **Playbook Launch Reliability**: Restored the playbook list to the working card/group flow from `0.6.14`, so starting a playbook once again routes through the expected `playbookId` launch path instead of breaking from the list view.
- **Agent Session Attachment Stability**: Fixed the agent attachment provider wiring so active agent chats no longer crash with `useAgentSessionState must be used within AgentSessionProvider` while loading session-scoped attachments.
- **Session Attention Acknowledgement Race**: Hardened session viewed-state persistence so acknowledging a session no longer clears newer attention events that arrive between the read and update steps.

### 🔧 Internal

- **Release Validation Cleanup**: Cleaned up Unix-gated session workspace cwd test imports so Windows release validation no longer emits avoidable Rust warnings during the patch pipeline.

## [0.6.14] - 2026-03-22

### 🐛 Fixes

- **Cross-Platform Validation Stability**: Fixed session workspace cwd validation so equivalent macOS temp paths like `/var/...` and `/private/var/...` no longer fail `refactor:validate`, restoring green CI behavior for macOS and the dependent Windows matrix leg.

### 🔧 Internal

- **Release Validation Cleanup**: Updated stale workspace file-operation tests and Unix-only PATH helper coverage so the Rust validation pipeline passes cleanly during patch releases.

## [0.6.13] - 2026-03-22

### 🐛 Fixes

- **Linux Dock Launch MCP Recovery**: Restored full executable PATH discovery for GUI-launched sessions on Linux and other Unix desktops, so `npx`/`uvx`-based MCP servers can start reliably even when LibrAgent is launched from a dock or app shortcut instead of a terminal.
- **Session MCP Workspace Consistency**: Session-isolated stdio MCP servers now create and start inside their session workspace, keeping relative file access and startup behavior aligned with the active workspace instead of the app's inherited working directory.
- **Grok Extension Setup Prompting**: The bundled Grok MCP preset now declares its required `XAI_API_KEY`, so extension setup no longer leaves the server underconfigured.

### 🔧 Internal

- **PATH and Session Spawn Regression Coverage**: Added targeted integration coverage for effective PATH recovery, persistent shell startup, and session MCP workspace cwd behavior to keep release builds honest.

## [0.6.12] - 2026-03-22

### 🚀 Features

- **Faster MCP Server Saves**: MCP server saves now complete immediately and verify in the background, so the dialog no longer blocks on slow dry runs.

### 🐛 Fixes

- **Verified Server Card Feedback**: Server cards now show pending, success, and error states from persisted verification data, including the last dry-run error when a server fails validation.
- **Fresh Tool Metadata After Verification**: Tool counts and cached tool lists now refresh after verification completes, keeping the server card and tool modal in sync with the latest server state.
- **Gemini Session Startup Stability**: Fixed the session-isolated stdio launch path so `npx`-based Gemini servers start from the correct working directory instead of failing during initialization.

### 🔧 Internal

- **MCP Verification Plumbing**: Added persistence for verification state, background verification events, and supporting tests so server metadata updates stay consistent across reloads.

## [0.6.11] - 2026-03-21

### 🚀 Features

- **Unified Workspace Search**: Replaced the split workspace search flow with a single search tool that can combine file-name filtering and in-file text matching, making agent search requests simpler and more predictable.
- **Clearer Agent Session Feedback**: Session messaging and status checks now expose richer progress details such as message IDs and turn counts, making delegated agent work easier to track.
- **Explicit Todo Actions**: Planning todos now use clearer action-based updates so agents can mark items done, reopen them, or cancel them without relying on ambiguous boolean toggles.

### 🐛 Fixes

- **Tool Surface Cleanup**: Removed obsolete UI tool paths and dead templates so `presentInteractive` remains the single public UI entry point while internal callbacks stay intact.
- **Sharper MCP Server Guidance**: Simplified external MCP server registration and discovery text so tool descriptions focus on intent, while transport-specific details stay in the schema fields where they belong.
- **Agent Error Guidance Consistency**: Tightened agent/session guidance text and validation messaging so missing configuration errors now point more clearly to valid `agentId` usage and recovery steps.

### 🔧 Internal

- **Workspace File Operation Consolidation**: Merged older workspace search implementations into a cleaner shared path and removed dead file-operation modules.
- **Builtin Registry Wording Cleanup**: Corrected misleading builtin tool registry comments and refreshed success hints around external server attachment flows.

## [0.6.10] - 2026-03-21

### 🐛 Fixes

- **Linux OTA Update Safety**: In-app update installs are now blocked for non-AppImage Linux installs that cannot safely overwrite the current executable. LibrAgent now shows clearer permission guidance and keeps the update toast concise with a changelog link instead of dumping long release text into the notification.
- **Session Viewed-State Efficiency**: Active agent sessions now update unread/viewed state locally during conversations and only persist viewed timestamps at meaningful moments like session entry and app refocus, reducing unnecessary backend writes while keeping the UI responsive.

## [0.6.9] - 2026-03-21

### 🐛 Fixes

- **Persistent State Recovery**: Fixed a critical regression in `0.6.8` where some upgrades or fresh installs could appear to wipe existing sessions, messages, planning data, settings, and MCP server entries by booting against a fresh empty database. The app now restores preserved user data automatically when a quarantined database is available.
- **Safer Database Startup**: Tightened database startup recovery so LibrAgent no longer quarantines an existing database for generic initialization failures like transient locks or non-structural startup issues, reducing the risk of false "data loss" scenarios.

### 🔧 Internal

- **Windows CI Build Quoting**: Fixed the Tauri build step in GitHub Actions so Windows release jobs pass valid JSON config overrides during CI builds.

## [0.6.8] - 2026-03-21

### 🚀 Features

- **Scheduled Task Mention Autocomplete**: Scheduled task prompts now support workspace-scoped `@skill:` and `@file:` completion when a workspace override is set, making it easier to reuse local skills and files without leaving the modal.

### 🐛 Fixes

- **Scheduled Task Workspace UX**: Fixed workspace directory drag-and-drop handling and reduced modal copy so long workspace paths no longer break the scheduled task UI.
- **Scheduled Task Timezone Safety**: Preserved local-time scheduling behavior while keeping legacy UTC tasks compatible, so daily/weekly/monthly schedules render and run consistently.

### 🔧 Internal

- **Workspace Reference Plumbing**: Extended the reference and autocomplete pipeline to support workspace-root file lookup for scheduled task prompts alongside the existing session workspace path flow.

## [0.6.7] - 2026-03-21

### 🚀 Features

- **Modern Design System Unification**: Comprehensive UI overhaul across all agent and extension views to provide a consistent "Modern Terminal" experience.
  - **Draft Chat View**: Redesigned pre-session interface with a floating translucent input area, three-column layout (matching active chat), and enhanced profile cards.
  - **Assistant Hub & Cards**: Standardized typography (`font-sans` for readability) and icon styles. Assistant selection cards now feature glassmorphism effects and refined hover animations.
  - **Extension Management**: Reordered MCP server view to prioritize installed extensions and added clear visual separation from recommended presets.
- **Session Attention & Notifications**: Improved user feedback with session attention acknowledgement and automatic unread state clearing for a smoother multi-session workflow.

### 🐛 Fixes

- **Batch File Import**: Refactored `importFile` tool to `importFiles`, allowing multiple files to be imported in a single efficient operation with individual result tracking.
- **Sidebar UX & Navigation**: Fixed clickability issues and hit areas for the collapsed sidebar. Aligned header split lines across the sidebar and main view for pixel-perfect layout.
- **Tauri v2 Updater**: Re-enabled artifact generation for the Tauri v2 updater and updated GitHub Actions for reliable production releases.
- **Translation Stability**: Fixed a runtime error in `AgentChatStartView` caused by a missing translation variable.

### 🔧 Internal

- **Sidebar Consolidation**: Optimized vertical space by consolidating settings and version display into a single row.

## [0.6.6] - 2026-03-21

### 🚀 Features

- **Enhanced Session UX**: Implemented SP23 session UX improvements and attention notifications for better user feedback during active workflows.
- **Accessibility Improvements**: Added `aria-label` to `ToolCallCompactItem` to improve screen reader support for tool execution summaries.

### 🐛 Fixes

- **[CRITICAL] SQL Injection Prevention**: Hardened `column_exists` validation by strictly validating `table_name` to prevent potential SQL injection vulnerabilities.
- **Database Validation**: Added unit tests and improved robustness for column existence checks in the database layer.

### 🔧 Internal

- **Agent Status Bar Optimization**: Refactored `AgentChatStatusBar` to use "Adjusting State During Render" pattern instead of `useEffect`, reducing unnecessary re-renders and improving UI stability.
- **Documentation Updates**: Added a comprehensive Navigation Guide for UI Routes to improve developer onboarding and codebase navigation.

## [0.6.5] - 2026-03-19

### 🚀 Features

- **Multilingual Documentation**: Added comprehensive documentation in Korean, German, Spanish, French, Japanese, Portuguese, and Chinese to support a global contributor base.
- **Workspace `writeFile` Append Mode**: Introduced a new `mode` parameter to the `writeFile` tool, allowing agents to append content to existing files without overwriting them.
- **i18n Regional Support**: Enhanced language detection to correctly handle regional variants (e.g., `zh-CN`, `ko-KR`) by resolving them to their base language tags.

### 🐛 Fixes

- **[CRITICAL] SQL Injection Protection**: Replaced dynamic SQL construction with parameterized queries across the search and validation layers to prevent SQL injection vulnerabilities.
- **Browser Process Cleanup**: Implemented explicit termination of all interactive browser webview processes upon application exit, ensuring no "zombie" processes remain alive after the main window is closed.
- **Agent Prompt Latency**: Optimized the construction of agent service contexts and reduced message pre-processing overhead, resulting in lower chat interaction latency.
- **macOS Build Compatibility**: Fixed platform-specific import errors on macOS by correctly gating `OnceLock` and related symbols behind target OS flags.

### 🔧 Internal

- **Memory Optimization**: Reduced unnecessary heap allocations in the Rust backend by replacing `.collect::<Vec<_>>().len()` patterns with O(1) `.count()` calls on iterators.
- **Accessibility Enhancements**: Added `aria-label` and `title` attributes to chat input controls and improved focus-state visibility for assistive technologies.
- **Community Health**: Added standardized Issue and Pull Request templates and refined the contributor setup guides to improve the developer experience.

## [0.6.4] - 2026-03-18

### 🐛 Fixes

- **macOS MCP Server Launch**: Fixed `npx`/`node` commands failing with "No such file or directory" when launching MCP servers from the macOS `.app` bundle. GUI apps inherit a minimal `launchd` PATH that strips nvm, Homebrew, and Volta — the fix queries the user's login shell once at startup to restore the full PATH for all child processes.
- **Spending Cap Retry Loop**: Unified spending-cap error detection into a shared utility with more robust message parsing (handles plain error objects in addition to `Error` instances), ensuring no-retry behaviour is applied consistently across the retry and fallback paths.

### 🔧 Internal

- **`isSpendingCapError` Utility**: Extracted duplicate spending-cap detection logic from `useLLMListener` and `base-service` into a single shared function in `ai-service/utils.ts`, and added dedicated unit tests.

## [0.6.3] - 2026-03-18

### 🚀 Features

- **Workspace Skills Integration**: Introduced integration for publishing workspace skills, allowing agents to better utilize and share specialized capabilities within a workspace environment.

### 🐛 Fixes

- **macOS Code Signing & Notarization**: Implemented full macOS code signing, entitlements, and notarization configuration, ensuring that universal macOS builds are properly signed and verified for distribution.
- **Scoped Skill Loading**: Fixed skill content loading to support scoped access, ensuring that skills are resolved correctly within their intended context.
- **CI Pipeline Stability**: Pinned the `tauri-action` version in the release workflow to prevent breaking changes from upstream actions and ensure stable production builds.

### 🔧 Internal

- **Weaver Modal Refactoring**: Refactored the Modal component in the Weaver feature using the Compound Components Pattern, improving code structure and making the component more extensible.
- **Enhanced Test Coverage**: Expanded unit and integration test coverage for core schemas and models (Sonar) and optimized validation coverage by removing duplicate test cases.
- **Documentation Polish**: Reorganized Weaver's modal journal entries and documentation to reflect recent architectural improvements and provide a clearer logical flow.

## [0.6.2] - 2026-03-18

### 🐛 Fixes

- **Smarter Retry Handling**: Tightened LLM retry behavior so the app no longer performs a pointless final backoff after exhausting retries, and it now stops retry/fallback loops immediately when providers return spending-cap billing errors that cannot recover automatically.
- **Windows Terminal Stability**: Preserved the `CREATE_NO_WINDOW` launch flag for the persistent shell on Windows, preventing stray terminal window flicker during background command execution.

### 🔧 Internal

- **macOS Release Pipeline**: Removed the redundant legacy macOS-only release workflow and upgraded the main release automation to the current Tauri action so universal macOS builds no longer rely on stale notarization behavior.

## [0.6.0] - 2026-03-17

### 🚀 Features

- **Scheduled Task Workspace Control**: Scheduled tasks can now target a specific workspace folder. You can browse for a directory or drag and drop one directly in the task modal, and the configured workspace is shown in the task list for easier review.
- **Persistent Scheduled Session Continuity**: Scheduled tasks now resume their pinned agent session reliably instead of silently drifting to a fresh session after restart or lazy unload. If the old session is truly gone, the task recreates it using the same pinned ID.

### 🐛 Fixes

- **Async Command Environment Isolation**: Closed environment leakage gaps in async command execution and fixed the related Windows regression so isolated processes keep the right minimal environment without inheriting sensitive host state.
- **MCP Server Card Actions**: Refined the server card action layout so management controls behave more predictably and remain usable in tighter UI states.

### 🔧 Internal

- **Scheduled Task Data Model**: Extended the scheduled task persistence layer, migration set, and backend wrappers to carry workspace overrides end to end while keeping validation and release checks green.

## [0.5.38] - 2026-03-17

### 🚀 Features

- **Planning Panel Localization**: Localized the Agent Planning panel in both English and Korean, covering the active goal, task list, and priority labels for a more consistent multilingual agent experience.
- **Scheduled Task Editing UX**: Improved the scheduled tasks workflow so task edits now handle assistant selection more reliably and behave more predictably under rapid user interaction.

### 🐛 Fixes

- **Scheduled Task Reliability**: Fixed follow-up issues in the scheduled tasks flow by persisting assistant changes correctly, tightening duplicate action guards, and surfacing clearer errors during toggle/delete operations.
- **MCP Dialog Loading UX**: Fixed MCP server dialogs so the close button is hidden while loading or saving, preventing misleading clicks on controls that cannot safely close yet.
- **Cross-platform Path Safety**: Restricted path separator normalization to Windows-only code paths, preventing valid Unix filenames containing backslashes from being corrupted.
- **Agent Status Bar Stability**: Removed a render-phase state reset in the agent status bar to avoid React concurrency and StrictMode issues when switching sessions.

### 🔧 Internal

- **Assistant Model Cleanup**: Removed duplicated assistant config definitions and continued decoupling assistant DTOs from Tauri command handlers to keep the backend model layer cleaner.
- **Backend Wrapper Test Coverage**: Added focused tests for core backend wrapper modules and refreshed release-adjacent formatting and review follow-ups to keep CI stable.

## [0.5.37] - 2026-03-17

### 🚀 Features

- **Autonomous Scheduled Tasks**: Introduced **YOLO Mode** for scheduled tasks, allowing agents to execute sensitive tools automatically without manual approval.
- **YOLO Status Indicators**: Added high-visibility YOLO badges to the scheduled tasks list and a dedicated toggle in the task configuration modal for easier autonomous workflow management.

### 🐛 Fixes

- **Startup Race Condition**: Resolved a critical issue where scheduled tasks triggered during application startup would fail to reach the frontend. Implemented a robust **Frontend-Ready Handshake** that ensures the UI is fully initialized before the backend fires automation events.
- **LLM Request Stability**: Enhanced session state validation to prevent redundant or stale LLM completion requests during rapid state transitions.

### 🔧 Internal

- **Parameter Object Refactoring**: Applied the Parameter Object pattern to `ScheduledTaskRepository` and service layers, improving code maintainability and ensuring compliance with strict Rust linting rules.
- **Repository Abstraction**: Refined the repository interfaces to better handle complex multi-argument operations without sacrificing type safety or readability.

## [0.5.36] - 2026-03-17

### 🚀 Features

- **Unified Agent Domain (SP22-2)**: Implemented a robust unified agent domain using Rust-to-TS SSOT (Single Source of Truth) codegen. This centralizes built-in service definitions, ensuring perfect type safety and consistency between the backend registry and frontend clients.
- **Enhanced Agent Discovery**: Improved specialist agent discovery with human-readable capability summaries. Agents can now discover and reason about available capabilities including external MCP servers directly within their context.
- **System Capability Catalog**: Added a comprehensive capability catalog to the agent context, providing models with a clear map of available system tools and platform features for better task planning.
- **Extended MCP Timeouts**: Increased the default timeout for sub-agent sessions to 1 hour, supporting complex multi-agent workflows that require extended execution windows.
- **i18n & Localization Expansion**: Significantly expanded English and Korean localization across the `AgentToolsModal`, `AgentModelPicker`, `AgentDraftChatView`, and Settings panels.
- **Accessibility & UX**: Added keyboard focus states to all native buttons (Palette) and enhanced the responsiveness and accessibility attributes of the `ScheduledTasksPage`.

### 🐛 Fixes

- **[CRITICAL] Environment Variable Leakage**: Resolved multiple security vulnerabilities (Sentinel) where sensitive host environment variables could leak into terminal launcher and system utility processes.
- **Duplicate Tool Call IDs**: Fixed an issue where certain LLMs would hallucinate duplicate tool call IDs, causing React key collisions and preventing tool results from being correctly associated with messages.
- **UI Flickering & Stability**: Resolved text bubble flickering by synchronizing message IDs and implemented functional state updates to fix intermittent tool expansion failures.
- **Windows Path Normalization**: Fixed shell execution errors on Windows by normalizing backslashes and ensuring consistent CWD handling across platforms.
- **Message Transition**: Ensured smoother message transitions and more accurate tool status displays during active streaming.

### 🔧 Internal

- **Architectural Decoupling (Nexus/Fractal)**: Continued the modularization effort by decomposing `AgentDraftChatView`, `agent/workflow`, and `agent/llm/completion` into focused sub-modules to improve maintainability and build times.
- **Dependency Inversion**: Completed dependency inversion for core domain services, enabling better isolation and more reliable unit testing.
- **Expanded Test Coverage**: Added comprehensive test suites for `RustAssistantService`, `AIServiceFactory`, `Settings` backend, and `Message` service, reaching higher reliability across the IPC boundary.
- **CI/CD Maintenance**: Upgraded the CI environment to Node.js v20 and synchronized lockfiles for improved dependency stability.

## [0.5.35] - 2026-03-14

### 🚀 Features

- **Context Compaction Optimization**: Re-engineered context management to use a Rust-owned compacting state, significantly improving conversation length stability. Added Sonner toast notifications for compaction events and structured markdown summaries for better transparency.
- **Media Tools Expansion (SP21)**: Introduced the `MediaServer` builtin MCP tool, enabling agents to "see" and "listen" to multimedia content. Media results are now intelligently injected into the LLM context across supported providers.
- **UI Resource UX Improvements**: Optimized the UI Resource presentation with enhanced copy-to-clipboard functionality and support for "simple mode" to reduce visual clutter.
- **Assistant Management Refactor**: Migrated the Assistant List and Skills Editor to a custom hook pattern with SWR integration, ensuring declarative data fetching and eliminating search UI flashing.

### 🐛 Fixes

- **[CRITICAL] Environment Variable Leakage**: Fixed multiple security vulnerabilities where host environment variables could leak into spawned processes during bootstrap platform detection and `command_exists` checks.
- **Compaction Stability**: Resolved a migration bug in the compaction logic and added token calibration to ensure precise context window management.
- **Session History Performance**: Enhanced accessibility and data integrity in the `SessionHistoryPanel` by implementing deferred rendering and fixing SWR mutation leaks.
- **Race Conditions**: Resolved several race conditions in assistant hooks and strengthened IPC boundary reliability during rapid tool-call aborts.

### 🔧 Internal

- **Architecture Decoupling**: Refactored the monolithic Workspace terminal handler and Assistant module into focused sub-modules, improving maintainability.
- **AI Service Generics**: Applied a generic pattern to `BaseAIService` for type-safe message and tool conversions across all providers.
- **Built-in Tool Consolidation**: Unified internal tool routing and consolidated legacy built-in tool definitions.
- **Test Coverage (Sonar/Hermes)**: Expanded the regression test suite covering compaction flows, date-utils coverage, and IPC boundary error handling.

## [0.5.34] - 2026-03-10

### 🐛 Fixes

- **Keyboard Focus on Copy Button**: The copy button in markdown message blocks was invisible to keyboard users due to `opacity-0` with no `focus-visible` override. Added `focus-visible:opacity-100` and standard focus ring styles so the button is visually apparent when tabbed to.

### ⚡ Performance

- **Anthropic Prompt Cache Hit Rate**: Service context sections are now appended to the system prompt in deterministic (sorted) order, eliminating cache misses caused by random HashMap iteration order across requests. The tools list is also now marked with Anthropic's `cache_control: {type: 'ephemeral'}` to activate the second cache breakpoint, reducing cost and latency for tool-heavy agents.

## [0.5.33] - 2026-03-10

### 🚀 Features

- **Workspace Override Persistence**: Agent sessions now remember their custom workspace path across restarts. The override is persisted to the database and automatically restored when a session resumes.

### 🐛 Fixes

- **Windows SQLite Paths**: Fixed hardcoded `sqlite://` URL bindings that silently failed on Windows due to backslash path separators. A dedicated `format_sqlite_url()` helper now handles cross-platform path formatting correctly.
- **Message Token History**: Fixed `deserializeMessage` silently dropping historical token usage data — usage information is now correctly preserved when loading past messages.
- **Type Safety (OpenAI/Gemini)**: Replaced an unsafe double-cast on `chunk.usage` in the OpenAI streaming path with a proper runtime type guard; added the missing `thoughtsTokenCount` field to `TokenUsage.details` to eliminate cast workarounds in the Gemini path.
- **i18n Agent Drop Hints**: Workspace drag-and-drop hint text and toast error messages in the Agent Draft view are now fully localized (Korean + English).
- **Message Role Cast**: Role values with unexpected raw types are now safely cast to `Message['role']` instead of being widened to `string`.
- **Release Script Safety**: The release script now aborts immediately when `TAURI_SIGNING_PRIVATE_KEY` is not set, preventing silent signing failures during production builds.

### 🔧 Internal

- **Code Quality (PR #797)**: Applied all reviewer feedback — removed redundant `<TooltipProvider>` wrapper (already internal to `Tooltip`), fixed inline `import()` type annotations, updated docstrings for allowlist behavior accuracy, and ensured `deserializeMessage` maps all fields.
- **Message / Workflow Decoupling**: Extracted message queuing, DB persistence, and event emission out of the workflow orchestration loop into a dedicated `MessageService`.
- **UI Hook Refactoring**: Refactored `ServerToolsModal`, `EnvVarsForm`, and `HttpForm` to use custom hooks and a callback-ref pattern, reducing component complexity.
- **Interactive Handler Sub-modules**: Reorganized interactive code-execution handlers into focused sub-modules with cleaner visibility boundaries.
- **Test Coverage**: Added unit tests for backend wrappers, `parseAssistant`, `isValidMessage`, `useBuiltinTools`, and `useServerTools` hooks; Rust `format_sqlite_url` covered by integration tests.

## [0.5.32] - 2026-03-10

### 🚀 Features

- **Prompt Cache & TPS Metrics**: Implemented real-time tracking for prompt cache hit rates and generation speed (TPS). The UI now displays a "Zap" icon for cache hits and a "Gauge" for speed, providing deeper insight into LLM performance.
- **Model Picker Safety**: The model and provider selection controls are now automatically disabled when a session is not idle (busy/thinking), preventing invalid configuration changes during active workflows.

### 🐛 Fixes

- **Session Config Sync**: Fixed a UI synchronization issue where manual model switches wouldn't reflect correctly in the session state until a refresh; local state now stays in sync immediately after backend updates.

### 🔧 Internal

- **Smart Metrics Merging**: Re-engineered the metrics update pipeline to use "smart merging" and render-time derivation. This preserves previously known metadata (like TTFT and durations) even if new streaming chunks are incomplete, ensuring a flicker-free UI.
- **Code Quality & PR Feedback**: Addressed review comments for PR #794 and #796, including replacing inline imports with top-level types, migrating state updates to `useEffect` to avoid render anti-patterns, and improving logging accuracy.

## [0.5.31] - 2026-03-08

### 🚀 Features

- **YOLO Mode Inheritance**: Subagents spawned via `spawnAgent` now automatically inherit the parent's YOLO mode state, ensuring autonomous operation is consistent across the entire agent hierarchy.
- **Compact Context as Default**: The default context management strategy is now "compact" (48K token window), enabling longer conversations out of the box without manual configuration.
- **editFile Enhancements**: Renamed `replaceLines` to `editFile` with explicit action types (`REPLACE`, `INSERT_AFTER`, `DELETE`) and line 0 insertion support for prepending content; `endHash` staleness validation on both range boundaries is now enforced.
- **sampleText() for All Providers**: All 8 LLM provider services now implement `sampleText()`, enabling uniform provider capability checks across the platform.
- **Message Context Management**: Improved context handling for Gemini and Anthropic services, with smarter caching strategies for long conversations.
- **Typed IPC Invocations**: All Tauri command invocations are now fully typed end-to-end, eliminating runtime type surprises.
- **MCP Tool Quality**: Improved agent-facing guidance, aligned tool statuses, and removed ambiguous parameters from `spawnAgent` schema (system-level params `maxDepth`/`maxFanout` removed, ownership model enforced).

### 🐛 Fixes

- **Message Cache Off-by-One**: Fixed `get_page(page=0)` in message cache loading — now correctly uses `page=1` per the 1-based repository contract, preventing the first message from being silently skipped.
- **Assistant Tool Cleanup**: Removed `deleteAssistant` and `offset` parameter from agent-facing tools; removed duplicate tool registration in `mcp_manager`.
- **Shell Isolation**: Improved Unix shell isolation to use `C.UTF-8` locale and preserve proxy environment variables correctly.
- **Error Handling**: Enhanced error messages and structured error handling across Planning, Browser, Workspace, and Knowledge MCP components.

### 🔧 Internal

- **Integration Tests**: Moved `#[cfg(test)]` unit tests from lib source files to `src-tauri/tests/` (integration tests) so they actually run in CI (`cargo test --tests`); added `compact_context_repository` and `persistent_shell` integration test coverage.
- **i18n Test Stability**: Test suite i18n init is now synchronous (`initImmediate: false`), eliminating flaky `NO_I18NEXT_INSTANCE` warnings.
- **Accessibility**: Added keyboard focus and ARIA attributes to `ToolCallGroup` toggle; localized sidebar "Session" prefix and Scheduled Tasks strings.
- **Frontend Code Quality**: Fixed inline `import()` types in interfaces (replaced with proper top-level imports) and migrated remaining `console.*` calls to the centralized logger.
- **Persistent Shell Refactor**: Reorganized persistent shell module into sub-modules with improved visibility, documentation, and test extraction.

## [0.5.30] - 2026-03-06

### 🚀 Features

- **YOLO Mode Persistence**: YOLO mode (autonomous operation) is now persisted in the Rust backend and synchronized with the UI, including improved tool approval behavior.
- **Multimedia & Video Support**: Enhanced tool specs to support video content and improved multimedia rendering in the UI.
- **Fluid UX Improvements**: Enhanced responsiveness and UX in the Scheduled Tasks page and modals.

### 🐛 Fixes

- **[CRITICAL] Shell Security**: Fixed environment variable leakage in the persistent shell and implemented isolated environments for shell execution.
- **Windows Shell Stability**: Added UTF-8 BOM to PowerShell scripts to prevent hangs with non-ASCII characters and improved cross-platform shell type handling.
- **Architectural Decoupling**: Decoupled command handlers (Nexus) and refactored search logic into sub-modules (Fractal).
- **Workspace Security**: Improved security for workspace exports and addressed path traversal/validation in tool arguments.

### 🔧 Internal

- **Accessibility & i18n**: Added ARIA attributes to BaseBubble buttons and addressed localization feedback.
- **Code Quality**: Removed derived state in favor of declarative state during rendering and standardized JSDoc documentation.
- **Tool Consolidation**: Consolidated legacy shell tools into a unified PowerShell-based execution model for Windows.
- **Documentation**: Updated documentation for core libraries and hooks (Scribe).

## [0.5.29] - 2026-03-06

### 🚀 Features

- **Agent Session Manager**: Introduced `AgentSessionManager` to consolidate agent session lifecycle management, tool approvals, and robust background cleanup services.
- **Scheduled Tasks UX**: Improved localization (i18n) and screen reader accessibility (ARIA labels) for the Scheduled Tasks feature.
- **Refactoring Skill**: Added `refactor-builtin-tool` skill for AI agents to enforce context economy and design principles.
- **Weaver's Journal**: Created a dedicated journal to document refactoring patterns and architectural improvements.

### 🐛 Fixes

- **Logger Recursion Loop**: Reverted `safeInvoke` usage in `TauriLogFileManager` to prevent potential infinite recursion during logging.
- **Session Deletion Logic**: Decoupled session deletion domain logic from the UI framework (Nexus).

### 🔧 Internal

- **MCP Validation Sync**: Enforced synchronous validation for external MCP server registration, ensuring database state integrity and improving `listTools` visibility.
- **Tool Consolidation**: Merged `exportFile` and `exportZip` into a unified `export` tool, reducing duplication and AI cognitive load by over 200 lines.
- **UI Tool Context Economy**: Removed internal callback tools (`getUserAnswer`, `circuitBreak`, etc.) from the AI context to prevent hallucinated tool calls.
- **Logger File Management**: Optimized IPC boundaries for Logger File Management (Hermes).
- **Design Principles**: Added comprehensive documentation on built-in tool design principles and anti-patterns.
- **Task Components State**: Cleaned up and removed unnecessary derived state from scheduled tasks UI components.

## [0.5.28] - 2026-03-05

### 🚀 Features

- **YOLO Mode & Blocking Tool Approvals**: Implemented a blocking UX for tool execution approvals, allowing users to explicitly allow or block tool calls in-flight. Introduced "YOLO Mode" for autonomous operation.
- **Windows Process Stealth**: Added `CREATE_NO_WINDOW` flag to all spawned processes on Windows, preventing terminal flash/flicker during MCP server verification and tool execution.

### 🐛 Fixes

- **Tool Approval Fallback**: Corrected the fallback configuration for tool approvals, ensuring agents respect user settings when explicit approval config is missing.
- **Cross-platform Path Normalization**: Improved path handling and normalization across Windows, macOS, and Linux for file operations and database backups, resolving issues with mixed separators.

### 🔧 Internal

- **Fractal: Handler Refactoring**: Refactored `src-tauri/src/server/handlers.rs` into focused sub-modules, reducing visibility of internal state and improving maintainability.
- **Atlas: Pathing & Backup Alignment**: Unified cross-platform alignment for file system pathing and backup operations.
- **Test Coverage**: Added regression tests for tool approval fallbacks and path normalization.

## [0.5.27] - 2026-03-04

### 🐛 Fixes

- **Cross-platform terminal pathing**: Fixed terminal launch path handling on Windows, macOS, and Linux — Windows now correctly normalizes separators for `cmd.exe /D`, macOS uses proper AppleScript string escaping for paths with special characters, and Linux drops unsafe string concatenation.
- **Browser window creation**: Extracted `CreateWindowParams` struct and corrected `MAIN_SEPARATOR_STR` usage, improving browser automation reliability across platforms.

### 🚀 Features

- **Accessibility — Playbook icon buttons**: Added `aria-label` attributes to icon-only buttons in the Playbook feature for improved screen reader support.

### 🔧 Internal

- **Interactive Browser Server decoupling**: Introduced `BrowserEnvironment` trait to isolate domain logic from the Tauri framework, with `TauriBrowserEnvironment` as the concrete adapter — improves testability and separation of concerns.

## [0.5.26] - 2026-03-03

### 🐛 Fixes

- **[HIGH] Security: Command injection in platform utilities**: Fixed a critical command injection vulnerability in cross-platform utility functions — shell metacharacters can no longer be injected through tool arguments on any platform.
- **Scheduled task timing accuracy**: Added a 1-second buffer to scheduled task execution timing and now records skipped runs, preventing drift and improving reliability on restart recovery.
- **Scratchpad guidance messages**: Improved user-facing guidance in the `listScratchpad` and `readScratchpad` tool responses for clearer agent feedback.
- **@mention reference size guards**: `@file:` and `@skill:` references now reject files larger than 100 KB and binary files instead of silently injecting oversized or garbled content into context.
- **Agent session proxy readiness**: Sessions started by the scheduler now wait for the MCP service proxy to be fully ready before invoking the LLM, preventing tool-not-found errors on fresh sessions.

### 🚀 Features

- **Native dialog UX**: Enhanced responsiveness and layout polish across native dialog action flows.
- **i18n — Agent chat fully localized**: Agent chat components (headers, labels, status indicators) are now fully translated with expanded EN/KO coverage.

### 🔧 Internal

- **MCP & Playbook service decoupling**: Architectural separation of MCP server management from Playbook service logic, reducing cross-cutting concerns and improving testability.
- **IPC boundary optimization**: File Manager commands and batched assistant upserts (`saveAll`) tightened for performance and type safety at the Tauri IPC boundary.
- **`@mention` resolution moved to Rust backend**: `@skill:` reference resolution removed from the frontend and unified with `@file:` and `@playbook:` handling in the backend's `ReferenceRegistry`, ensuring consistent resolution across all mention types.
- **Tool routing refactor**: Removed `builtin_` prefix from internal tool routing; tool names are now cleaner and consistent across the proxy layer.
- **Test coverage**: Expanded hook tests covering `useDebounce` and `useAgentTools`.
- **Dependency updates**: `chrono` → 0.4.44, `tempfile` → 3.26.0, `anyhow` → 1.0.102.

## [0.5.25] - 2026-03-02

### 🚀 Features

- **Scheduled Tasks**: Full cron-based task automation — create scheduled tasks that fire agent sessions on a schedule, with persistent session reuse and missed-task recovery on restart.
- **@playbook Mention**: Type `@` in the agent chat input to reference playbook entries directly, injecting structured context into agent turns.
- **OpenRouter Provider**: New OpenRouter integration with live model listing via the public metadata API, dynamic pricing, and context-length discovery — no hardcoded model list required.
- **Workspace `searchLines` Directory Support**: `searchLines` tool now accepts a directory path and recursively searches all files within it, not just individual files.

### 🐛 Fixes

- **Scheduled task session shows "Unknown Assistant"**: Sessions created by the scheduler now correctly inherit the assistant's display name from the database instead of defaulting to `"Unknown Assistant"`.
- **HTTP MCP transport fixes**: Resolved connection and session management issues in the HTTP MCP backend.
- **Dynamic model discovery**: Removed all hardcoded `supportsDynamic` provider allowlists; model listing is now fully dynamic across all providers including OpenRouter.

### 🔧 Internal

- **Assistant Editor UX**: Improved responsiveness and layout in the assistant editor panel.
- **MCP Server page i18n**: Localized subtitle and expanded EN/KO translation coverage.
- **`searchLines` refactor**: Improved readability and error handling in the workspace search logic, with added regression tests.
- **OpenRouter regression tests**: New test suite covering `listModels` API override and metadata parsing.

## [0.5.24] - 2026-03-02

### 🚀 Features

- **MCP Server Configuration UI**: New `EnvVarsForm` and `HttpForm` components for configuring MCP server environment variables and HTTP transport settings directly from the UI.
- **Session Files Viewer**: UI to browse and inspect files associated with an agent session, with full LLM execution context visibility.
- **Agent Skills Expansion**: New bundled skills for system setup, document processing (DOCX/PPTX/XLSX), and additional agent utilities including OOXML schema support.
- **AgentSessionManager**: Comprehensive Rust-side agent session manager handling lifecycle, workflow orchestration, and context management — completing the Rust-orchestrated agent V2 architecture.
- **In-memory session repository**: Ephemeral session support for lightweight, non-persisted agent workflows.
- **MCP Server Management**: Full MCP server lifecycle management with new UI components, form handling, and backend controls.
- **i18n expansion**: Hardcoded UI strings extracted to semantic translation keys across agent and settings panels.
- **Tooltips on icon buttons**: Icon-only buttons across the agent UI now show accessible tooltips for better discoverability.

### 🐛 Fixes

- **Image drag-and-drop to LLM (multimodal pipeline)**: Three layered bugs fixed that prevented dragged images from reaching the LLM:
  1. `AgentDraftChatView`'s MIME detection was missing all image/audio types (returning `application/octet-stream` on Linux/WebKitGTK).
  2. Draft attachment loop hardcoded `status:'workspace-only'` — no inline base64 encoding path existed at all.
  3. `message-preprocessor` skipped inline attachments instead of injecting `MCPImageContent`/`MCPAudioContent` blocks into `message.content` for the LLM.
- **`get_skill_content` arbitrary file read**: Skill content API now validates that the requested path is within the configured skills directory and points to a `SKILL.md` file before reading — prevents path traversal via Tauri IPC.
- **`toggleBookmark` stale closure**: Bookmark toggle now derives `newValue` from the current session state before the optimistic update so rapid double-toggles send the correct boolean to the backend.
- **Security: MCP environment variable leak**: Fixed environment variable leak in MCP server verification; isolated env is now correctly enforced.
- **Cross-platform path bugs**: Resolved pathing issues in handlers and scripts on Windows.
- **JSDoc parameter mismatch**: Corrected mismatched parameter documentation in workspace-sync-service.
- **i18n missing keys**: Added missing i18n keys and react-i18next test mock to fix test failures.

### 🔧 Internal

- **Centralized MIME utility** (`src/lib/mime-utils.ts`): Single canonical `getMimeTypeFromFilename()` replaces three diverged local implementations across the codebase.
- **Environment variable isolation refactor**: Centralized env-var isolation logic into a dedicated utility module shared across MCP server spawn paths.
- **IPC boundary optimization**: Tauri command handlers tightened for performance and type safety across all IPC boundaries.
- **`useAgentTools` replaces `useSessionTools`**: Removed the thin redundant `useSessionTools` hook; `AgentChatInput` now uses the more robust `useAgentTools` with Zod validation, cleanup guards, and error state.
- **Redundant `TooltipProvider` removed**: Eliminated nested `TooltipProvider` wrapping from icon button components.
- **Regression test suite expanded**: New tests for MIME utility (30 cases), inline image injection in message preprocessor (7 cases), `toggleBookmark` stale closure (4 cases), and Rust-side skill path validation (4 cases).

## [0.5.23] - 2026-03-02

### 🚀 Features

- **Multimodal LLM Support**: Added ability for agents to process and send images and audio directly through OpenAI, Anthropic, and Gemini models. Mapped files correctly to native schema inputs (e.g. `inlineData` for Gemini, `image_url` / `input_audio` for OpenAI).

### 🐛 Fixes

- **`useSettings` Context Fix**: Updated the test context mock to cleanly default to `DEFAULT_SETTING`, fixing downstream test failures and removing dead imports.
- **Double Submit Guards**: Buttons with async handlers across the UI now block synchronous double-clicks correctly.
- **Form UI Streamlining**: Cleaned up the interface types for `InputWithLabel`, `TextareaWithLabel`, and `Label` components.
- **Tool Call Execution Context Formatting**: Fixed the way `tool_calls` are shaped when injecting them into `useLLMExecution`.
- **UI Simple Mode Tool Call Display**: The raw `parsedArgs` for tool calls are now accurately passed in UI Simple Mode, supported by a newly added test coverage suite.

### 🔧 Internal

- **Expanded Test Coverage**: Strengthened test assertion coverage for MCP schema builder utilities and the `useSettings` hook.
- **Documentation Overhaul**: Updated the internal architecture and migration documentation for Agent V2 to clarify component isolation and state contexts.

## [0.5.22] - 2026-03-01

- **`@skill:` mention autocomplete in chat**: Draft chat now supports `@skill:` mention syntax with autocomplete, letting agents reference skills directly from the input field for faster workflows.
- **Session bookmarks (SP10)**: Sessions can now be bookmarked for quick access, with DB migration and full UI support added.
- **Skill mention reference system (SP11/SP12)**: Agents can now resolve and inject skill documentation via `@skill:` references directly into prompts, enabling richer context-aware interactions.
- **i18n: Session History Panel & SessionCard**: Session history UI components fully localized with Korean and English support.
- **Type-safe Tauri IPC generics**: Strict generic typing applied to all `invoke` calls, catching type mismatches at compile time and eliminating unsafe casts in the IPC layer.
- **Chat input consistency**: `AgentChatInput` harmonized with `DraftChatView` styling for a unified look across chat entry points.
- **Session creation UX**: Fluid enhancements applied to the session creation flow for improved responsiveness and polish.

### 🐛 Fixes

- **Security: workspace override path validation**: Agent service now validates that workspace override paths exist, are directories, and are accessible before accepting them — prevents invalid or malicious paths from being registered as session workspaces.
- **Security: Windows reserved filename blocking**: `SecurityValidator` now blocks Windows reserved filenames (e.g. `CON`, `NUL`, `COM1`) on write operations while still allowing deletion of such files.
- **Security: input validation hardened**: Whitespace-only MCP server names, excessively long paths, and oversized todo content are now rejected at the validation layer.
- **Swarm: agent self-termination prevented**: Agents can no longer accidentally terminate their own session via the swarm API.
- **Swarm: zombie child processes cleaned up**: `awaitAgent` now terminates stuck child sessions on timeout instead of leaving them as zombies.
- **Swarm: agent spawn/session bugs fixed**: Three bugs in agent spawn and session registration resolved, improving reliability of multi-agent workflows.
- **Swarm: error response quality**: HTTP status codes restored in error responses; internal API paths no longer leaked in error messages.

### 🔧 Internal

- **`AgentService` extracted**: Agent domain logic moved from Tauri command handlers into a dedicated `AgentService` for better separation of concerns and testability.
- **Cross-platform path handling**: Replaced hardcoded forward-slash path construction with `PathBuf::join` in browser content module for proper cross-platform behavior.
- **Windows linker fix (LNK1102)**: Switched to `debug=1` line table profiles (and `rust-lld` as intermediate step) to resolve OOM linker errors on Windows.
- **Expanded test coverage**: Agent chat utility tests expanded with broader coverage of edge cases (Sonar initiative).

## [0.5.21] - 2026-02-28

### 🚀 Features

- **MCP HTTP endpoint**: Builtin MCP tools are now exposed via an HTTP endpoint, enabling external integrations to call agent tools directly. A sessionless `POST /mcp` endpoint with auto session selection is also available for stateless callers.
- **Clear button in session history search**: Added a one-click clear button to the session history search input for faster navigation.
- **Reserved builtin name blocking**: The MCP server registration UI now blocks users from registering external servers with names that conflict with reserved builtin prefixes, preventing tool routing confusion.
- **Playbook & chat input UX improvements**: Fluid UX enhancements applied to `PlaybookList` and `AgentChatInput` for a smoother interaction flow.
- **i18n expansion**: Strings across the MCP, assistant, playbook, and settings pages extracted and localized with Korean and English support.

### 🐛 Fixes

- **Builtin tool names cleaned up**: Tool calls throughout the UI (AgentToolsModal, tool call groups, MCP HTTP exposure) now display `group / tool` format instead of the raw internal `builtin_group__tool` prefix, making logs and history readable.
- **Security: Unix shell env isolation**: Environment variable isolation implemented for Unix shell execution in MCP server processes — prevents host environment leakage into spawned tool processes.
- **Security: symlink traversal in SecurityValidator**: `SecurityValidator` now canonicalizes the base directory to prevent symlink-based path traversal attacks.
- **Windows path injection fixed**: Path handling in the UI corrected for Windows, and Linux file manager integration improved.
- **Legacy `content_store` alias handling**: Legacy `contentstore` aliases now correctly resolved, fixing compatibility with older sessions.
- **MCP HTTP structured_content stripped**: `structured_content` (a LibrAgent-internal field) is correctly stripped from responses exposed via the external MCP HTTP endpoint.
- **`spawnProcess` / `spawnAgent` response quality**: Tool text responses for process spawning now include actionable IDs and follow-up instructions so agents can act on results without ambiguity.
- **External server routing fallback**: `MCPServiceProxy` now falls back gracefully when `builtin_`-prefixed tool calls are routed to external servers, providing a correction hint rather than a hard failure.

### ⚡ Performance

- **Logger IPC batching**: Logger now batches IPC calls to reduce round-trips, significantly cutting log-related overhead in high-volume agent sessions.
- **Tool call argument parsing memoized**: Argument parsing in tool call components is now memoized to avoid redundant work on re-renders.

### 🔧 Internal

- **`AgentService` extracted from commands layer**: Agent orchestration logic moved out of Tauri command handlers into a dedicated `AgentService`, improving testability and separation of concerns.
- **`SessionMCPManager` modularized**: Refactored into focused lifecycle/execution/cleanup submodules.
- **Session services reorganized**: `SessionManager` split into `DirectoryService` and `CleanupService` for clearer responsibilities.
- **Attachments renamed from `content_store`**: Internal rename of `content_store` to `attachments` across Rust backend for clarity (Phase 1 & 2).
- **React agent components modernized**: Agent feature components updated to modern React patterns (adjust-during-render instead of sync `useEffect`).
- **Rust clippy warnings resolved**: `needless_borrow`, `clone_on_copy`, and related warnings cleaned up across the backend.

## [0.5.19] - 2026-02-25

### 🚀 Features

- **LLM retry with fallback model**: Agent LLM calls now automatically retry with exponential backoff and jitter on transient failures. A configurable fallback model can be specified so the agent degrades gracefully instead of hard-failing when the primary model is unavailable (SP4 implementation).

### 🐛 Fixes

- **mcp-hn server command corrected**: The `mcp-hn` preset now launches via `npx` with the correct package name, fixing MCP server startup failures for the Hacker News integration.
- **Service proxy routing error handling**: Improved error propagation in the MCP service proxy routing layer so failures surface with actionable messages rather than being silently swallowed.

### 🔧 Internal

- **`service_proxy.rs` modularized**: The monolithic `service_proxy.rs` split into focused submodules for improved maintainability and testability.
- **`MCPServerDialog` services extracted**: Service logic and UI components extracted from `MCPServerDialog` into dedicated modules, improving separation of concerns.
- **Download commands architecture**: Download-related Tauri commands refactored for better separation from UI concerns.
- **Outdated setup scripts removed**: Legacy Python/Node.js/uv installation and verification scripts pruned from the repository.

## [0.5.18] - 2026-02-25

### 🚀 Features

- **Settings page overhaul**: `SettingsPage` refactored with a dedicated `useSettingsForm` hook — eliminates cascading `useEffect` state duplication, adds proper dirty-state detection, and batches saves into a single optimized IPC command.
- **Settings i18n**: All Settings components (General, API Keys, etc.) are now fully localized with Korean and fallback English strings; save action labels and error messages no longer appear as raw i18n keys.
- **Workspace panel click-to-upload**: AgentWorkspacePanel now supports clicking anywhere on the drop zone to open a file dialog, improving accessibility and discoverability.
- **Assistant management service**: New `AssistantService` with local/remote source toggling backed by Rust commands, enabling agent-level assistant CRUD from the backend.
- **Linux file manager integration**: Cross-platform path handling and Linux file manager launch fully fixed, including correct `local_bin` resolution on non-standard `PATH` setups.

### 🐛 Fixes

- **[CRITICAL] Zip Slip vulnerability patched**: Skill import now uses `extract_zip_secure()` which validates paths via `enclosed_name()`, canonicalizes the target directory, and rejects symlink entries — preventing arbitrary file writes from malicious ZIP archives.
- **Settings retrieval uses `safeInvoke`**: Replaced bare `invoke` calls in settings fetch/update with `safeInvoke` for consistent error logging and handling.
- **GeneralTab error message formatting**: Improved error display formatting in the General settings tab for clearer user feedback.
- **Skills verification error handling**: Improved robustness of the skill verification flow to surface errors correctly.
- **Gemini provider mapping corrected**: `LLMConfigManager` now correctly maps the Gemini provider configuration, fixing model selection for Google AI users.

### ⚡ Performance

- **`handle_llm_response` deep-clone eliminated**: LLM response processing avoids unnecessary deep clones, reducing allocations on every turn of the agent loop.

### 🔧 Internal

- **MCP session isolation tests consolidated**: Session isolation integration tests merged into `stdio_manager` test suite for better co-location.
- **Integration test hardening**: Python3 availability checked before execution in integration tests; error message formatting standardized across test assertions.

## [0.5.17] - 2026-02-24

### 🚀 Features

- **Assistant session ID propagation**: Update and delete operations on assistants now correctly carry the session ID through the MCP builtin layer, enabling proper per-session assistant state management.
- **`SkillService` as a standalone service**: Skill resolution, metadata parsing, and CRUD management are now fully extracted into `services/skill_service.rs` — callable without a Tauri app context and independently testable.
- **Terminal command existence guard**: The terminal launch utility now proactively checks whether the target terminal command exists before spawning, producing a clear error instead of a silent failure.

### 🐛 Fixes

- **MCP server verification robustness**: The server management and verification flow has been refactored to eliminate race conditions and incorrect state transitions during server startup checks.
- **Download event type corrected**: `UpdateContext` was using a mismatched event type for download progress events, causing update notifications to behave incorrectly in certain cases.

### 🔧 Internal

- **Skill resolution refactored**: Scanning and resolution logic cleaned up for clarity and correctness, with new integration-level tests in `tests/skill_resolution_test.rs` and `tests/skill_parsing_test.rs`.
- **MCP server management modularized**: `mcp_commands.rs` and related management code restructured to separate concerns between command dispatch, connection lifecycle, and server registry.
- **`AssistantService` test hardening**: Comprehensive test suite added covering fallback scenarios, pagination edge cases, and mock fetch handling — including coverage for the new session ID paths.

## [0.5.16] - 2026-02-24

### 🐛 Fixes

- **Tool failure message now shows actual error**: When a tool call fails, the `guided_error` content from the tool result is now surfaced to the user instead of a generic "Unknown error" message, making debugging dramatically more actionable.
- **RecommendedPresets nested interactive controls**: Refactored `RecommendedPresets` to eliminate nested `<button>` / interactive element violations that caused broken click behavior in the MCP server preset UI.
- **Localized string defaults**: `SkillsListModal` and `GeneralTab` now fall back to sensible default values when i18n strings are missing, preventing blank labels in non-Korean locales.

### ⚡ Performance

- **`useMessageGrouping` text validation**: Whitespace detection in the message grouping hook is now short-circuit optimized, reducing unnecessary re-computation on large conversation threads.

### 🔧 Internal

- **`MCPServiceProxyManager` modularization**: The monolithic proxy manager is split into focused submodules (`caching`, `cleanup`, `creation`, `management`) with session locking via `creation_guards`, improving concurrency safety and testability.
- **`command_exists` extracted to shared platform utils**: Duplicate platform-detection logic consolidated into `src-tauri/src/utils/platform.rs` with tests, eliminating copy-paste across builtin server modules.
- **Skill resolution is now override-only (breaking change)**: `resolve_skills` previously merged global and assistant skills; it now returns _only_ assistant skills when an assistant has any, with a full fallback to global when none exist. Mixed/merged skill sets are no longer produced.
- **`SkillService` extraction**: Domain logic for skill resolution, metadata parsing, and management moved from `skill_commands.rs`/`skill_management.rs` into `services/skill_service.rs`, making it testable in isolation without a Tauri app context.
- **Built-in server definitions migrated**: Preset and skill resolution logic migrated to the new `mcp/presets.rs` backend structure for cleaner separation of concerns.

## [0.5.15] - 2026-02-23

### 🚀 Features

- **OTA Auto-Updater**: LibrAgent now checks for new releases in the background on startup. When an update is available, a non-blocking toast notification appears with "Install" / "Later" options. Updates download and install automatically; the app restarts on completion. Powered by `tauri-plugin-updater` against GitHub Releases.

### ⚡ Performance

- **Parallel MCP server initialization**: External stdio and HTTP MCP servers are now initialized concurrently using `JoinSet` instead of sequentially. With N servers configured, startup time now equals the slowest single server rather than N × slowest — a significant improvement once you start stacking up MCP servers.

## [0.5.14] - 2026-02-23

### 🚀 Features

- **Compass: Cross-platform terminal & path standardization**: Unified terminal launch and `PATH` environment variable handling across Windows, macOS, and Linux in the Workspace built-in server — eliminating `local_bin` resolution bugs on non-standard setups.
- **Session name previews the request**: Session name generation now incorporates a short preview of the initial user request, making session history far easier to scan at a glance.

### 🐛 Fixes

- **`mcp_manager` always enabled**: The `mcp_manager` built-in service was registered as `optional: false` in the registry but absent from `CORE_BUILTIN_SERVICE_ALIASES`, causing agents with an explicit alias list to receive "Built-in server 'mcp_manager' not enabled in this session" errors. Fixed in both the Rust registry array and the TypeScript `ServiceCategory` classification.
- **Assistant description silently lost on save**: `upsertAssistant()` manually assembled the save object but omitted `description`, `avatar`, and `disabledSkills` fields — every save (including toggling an MCP server) discarded them. All three fields are now preserved. Added a `TextareaWithLabel` description input to `AssistantEditor` General tab so users can set the field directly from the UI.
- **AI agents unable to set assistant description via MCP tools**: `createAssistant` and `updateAssistant` MCP tool schemas were missing the `description` parameter entirely. Both schemas now declare the field.
- **Workspace file listing resilience**: Individual entry errors during directory listing are now handled gracefully instead of aborting the entire operation.
- **Assistant validation schema**: `disabledSkills` is now correctly recognized as an optional field, preventing spurious validation errors when loading existing assistants.

### 🔧 Internal

- **Workspace command decoupling**: Business logic extracted from monolithic `workspace_commands.rs` into `services::WorkspaceService` (file listing, override management) and `utils::terminal` (cross-platform terminal launch). Command handlers now hold zero domain logic.
- **`AgentChatMessages` hook extraction**: Scroll management and file-refetching logic split into dedicated `useChatScroll` and `useFileRefetcher` hooks, reducing component complexity.
- **`ErrorBubble` memoization**: Component memoized with a stable `onRetry` callback to prevent unnecessary re-renders in `AgentChatMessages`.
- **Deprecated `call_tool_unified` removed**: Cleaned up the deprecated unified tool routing path and duplicate MCP tool registrations.
- **Regression tests**: Added `#[test]` cases for `mcp_manager` core alias correctness, assistant tool schema completeness (`createAssistant`/`updateAssistant`), and assistant serialization round-trip field preservation.

## [0.5.13] - 2026-02-23

### 🚀 Features

- **Background Tool Loading Readiness Signal**: MCP servers now emit a readiness signal after background tool registration completes. HTTP client timeout extended to accommodate slower server startups, reducing false-negative tool-not-found errors when agents start immediately after session creation.

- **Agent Topology Controls**: `AgentSessionMetadata` gains `maxDepth` and `maxFanout` fields for fine-grained control over sub-agent spawning limits. `RustMessage` structure enhanced with additional fields for richer IPC payloads between frontend and backend.

- **Accessibility**: Added accessible labels to `ServerCard` action buttons, improving screen-reader support across the MCP server management UI.

- **Localization**: `AppSidebar`, `ThemeToggle`, and `ErrorBoundary` are now fully localized (Korean + English), closing the remaining hardcoded-string gaps in the main shell UI.

### 🐛 Fixes

- **`ThrottlePromise` memory leak**: Multiple pending resolutions were not being flushed correctly, causing the queue to grow unbounded under rapid polling. Refactored to drain all waiting resolvers on each settled result.

- **Tool-level error silently reported as success**: `ToolExecutionResult.is_error` was derived only from the JSON-RPC protocol error field, meaning builtin tools that signal failure via `MCPResult.is_error` or `MCPContent::Text { is_error: Some(true) }` were emitting `ToolExecutionCompleted success=true`. Now checks all three failure signals.

### 🔧 Internal

- **`agent/llm/response.rs` fractal split**: The 1100-line response handler is split into three focused modules — `circuit_breaker.rs` (loop detection + tests), `tool_execution.rs` (sequential async tool dispatch), and the slimmed `response.rs` (orchestration only). No behavior change from the user's perspective.
- **Lazy `debug_content` serialization**: `serde_json::to_string_pretty` on tool results is now gated behind `log::log_enabled!(Debug)`, eliminating O(n) JSON serialization overhead on every tool call in release builds.
- **Agent IPC strict typing**: `agent_commands` optimized with strict TypeScript + Rust types — fewer `unknown` / `string` roundtrips across the IPC boundary.
- **Rate-limiting + session poll hints**: Session API rate-limiter improved with backoff hints for rapid-polling consumers.
- **MCP server management UI cleanup**: Removed stale dialog components superseded by the new MCP management flow.
- **Expanded test coverage**: `src/lib` utilities gain additional Vitest cases; `DroppedFileService` tests refactored with consistent formatting.

## [0.5.12] - 2026-02-22

### 🚀 Features

- **Blocking Process & Agent Waits (SP1)**: `waitForProcess` replaces the old polling-based `pollProcess` for long-running shell commands — agents now block on a `tokio::sync::Notify` and wake the instant the process finishes, eliminating redundant LLM round-trips and context-window bloat from status-polling loops. `awaitAgent` gains an indefinite-timeout mode (`timeout_seconds: 0`) for the same reason.

- **Max Concurrency Control (SP2)**: `ConcurrencyGate` enforces hard limits on parallel agent sessions (default: 4 active) and shell processes (default: 10). When a parent agent blocks on a child via `awaitAgent`, it suspends its active slot so the child can acquire one — preventing deadlocks. Limits are configurable in Settings → Advanced. Process completion now uses a push-notify system rather than fixed-interval polling.

- **Parent Session Cancel Isolation (SP6)**: Cancelling a parent session while it blocks waiting for a subagent now returns immediately instead of waiting up to 30 seconds for the next heartbeat. `cancel_workflow` fires a `SessionBus` notification on the parent's own bus entry; the `wait_until_session_terminal` loop subscribes to both the child and caller bus entries via a dual-notifier `tokio::select!`, so whichever fires first wins. Parent's concurrency slot is always released correctly even on the fast-cancel path.

- **Session Delete Options (SP7)**: Deleting a session with subagents now offers a choice instead of silently orphaning children:
  - **Delete all** — cascade-deletes the full descendant tree (BFS traversal).
  - **Delete only this** — removes the selected session and promotes its direct children to top-level sessions.
  - Single sessions (no children) see the original single-button confirm — no extra clicks.

### 🔧 Internal

- **Trace Analyzer Skill + `trace_dump.py`**: New dev tooling for analyzing agent session trace files — reports tool call frequency, concurrency patterns, and session outcomes directly from `.trace.json` files.
- **SP6 + SP7 Regression Tests**: `session_bus.rs` gains 4 `#[tokio::test]` cases covering dual-notifier wakeup, cancel flag short-circuit, and spurious-wake prevention. Frontend context tests add 4 Vitest cases covering BFS cascade removal and orphan promotion.

## [0.5.11] - 2026-02-22

### 🚀 Features

- **Playbook Server & Skills IPC**:
  - Added integration tests for PlaybookServer UI rendering and interaction flows.
  - Unified `SkillMetadata` type across frontend and backend for 1:1 parity with Rust.
  - Optimized `download_global_skills` Tauri command to stream download to file instead of memory.
  - Ported bundled skills from `dev/0.4.0` to `dev/0.5.x`.
- **Localization & Accessibility**: Localized MCP Server Management features (Korean and English) and enhanced a11y.
- **Performance**: Optimized `AgentToolCallGroup` rendering.
- **Security & Validation**: Enforced file size limits in `DocumentParser` and added validation tests for oversized files.
- **Test Coverage**: Expanded tests for date utils (locale-independent relative time) and retry utils.

### 🐛 Fixes

- **CSS Hierarchy**: Moved Pretendard `@import url()` before tailwindcss imports.
- **Cleanup**: Removed `download_global_skills` leftovers and related dead code from botched merge resolutions.
- **Test Mocks**: Made `toolResult` prop optional in `ToolCallCompactItem` test mock.

### 🔧 Internal

- **CI & Formatting**: Added Rust integration tests, improved CI workflow, and cleaned up code formatting in tools/tests for better readability.
- **Docs & Project Maintenance**:
  - Created `SECURITY.md`.
  - Added newline for better readability in Hermes's Journal.
  - Removed unused dependencies (`unit-prefix`) and bumped various versions (tempfile, indicatif, futures, uuid, tauri-plugin-http, regex, anyhow, docx-rs).

## [0.5.10] - 2026-02-21

### 🚀 Features

- **Browser `fetch` Tool**: New headless content extraction tool that lets agents fetch URLs and download files without opening a visible browser session — giving agents a lightweight scraping path alongside the existing interactive automation flow.

- **Declarative Builtin Service Registry**: `BUILTIN_SERVICE_REGISTRY` is now the single source of truth for all 12 builtin server names in both Rust (`agent/tools.rs`) and TypeScript (`runtime-builtins.ts`). Each server module declares `pub const NAME: &str` — the canonical name is defined exactly once and referenced everywhere, making name drift fail fast via regression tests rather than a silent routing failure.

- **Configurable Tool Call Detail Level**: Agent chat UI now supports user-selectable view modes for tool call rendering — Compact (simplified) and Developer (full arguments/results). Controlled per-session via settings.

- **Agent Start View Hub Layout**: Removed the inline session history panel from the start screen. Now a clean, centered assistant picker — history lives in the sidebar "Recent Sessions" and the dedicated History page where it belongs.

- **Assistant Card Redesign**: `AssistantSelectionCard` gets rounded-2xl corners, a dedicated icon badge, and a subtle lift-on-hover effect. More visual breathing room, clearer hierarchy.

- **Sidebar "See all sessions →"**: Recent Sessions section now has an explicit navigation item at the bottom of the list so users can reach the full History page without guessing.

### 🐛 Fixes

- **Circuit breaker consecutive-failure count was off-by-one**: The break trigger was firing one iteration late because the counter included the current (unsent) call in its total. `evaluate_circuit_breaker_count` now returns the historical failure count only; the break fires as soon as `consecutive >= 2`, matching original intent.

- **`content_store` canonical name mismatch**: `ContentStoreServer::name()` was returning `"contentstore"` (no underscore), causing tool routing to silently fall back through the alias layer. Fixed to `"content_store"` — the alias system would have masked this forever without the new regression tests.

### 🔧 Internal

- **i18n Settings Coverage**: All `settings.*` and `common.*` keys used in the Settings page now have proper translations in both `en` and `ko` locales. Previously the entire Settings UI was falling back to hardcoded English strings regardless of language selection.

- **Circuit breaker refactored** (`agent/llm/response.rs`): Extracted `evaluate_circuit_breaker_count` from the pre-processing loop. `count_consecutive_failed_calls` is now generic over a predicate closure, covering both same-tool-name and same-signature (tool + args) detection without duplication. `build_tool_call_indices` builds both `call_name_by_id` and `call_signature_by_id` maps in a single pass.

- **Alias layer removed**: The entire builtin-service alias indirection has been eliminated. `canonicalize_builtin_service_alias` (Rust) and `canonicalizeAlias` (TypeScript) are now single-scan / O(1) Set lookups against the canonical name list. No more shadow name tables to keep in sync.

- **4 registry regression tests** added to `tests/builtin_service_registry_tests.rs` — `each_builtin_server_name_is_in_registry`, `builtin_server_names_are_unique`, `registry_has_no_duplicate_canonicals`, `registry_and_server_list_are_in_sync` — protecting against future name drift across all 12 builtin servers. Tests run in CI via `cargo test --tests`.

### 📚 Docs

- Updated `create-builtin-tool` SKILL.md with the `pub const NAME` pattern, three-point registration checklist (`BUILTIN_SERVICE_REGISTRY` + regression test update + `mod.rs` wiring), and expanded Quality Gates.

## [0.5.9] - 2026-02-21

### 🚀 Features

- **Configurable Tool Group Visibility in Agent Renderer**:
  - `AgentToolGroupBlock` now respects the user setting `toolCallGroupVisibleCount` instead of hardcoded display limits.
  - Preserves performance optimization from memoized tool-group rendering while restoring end-user control from Settings.

- **Timeout Behavior Controls for MCP Execution**:
  - Added support to disable MCP tool execution timeout by default.
  - Added UI status indicator to make timeout mode visible to users.

### 🐛 Fixes

- **MCP Server Management Accessibility**:
  - Fixed install action semantics in preset cards with explicit button interaction wiring.
  - Added unique, descriptive `aria-label` text for environment variable/header removal buttons.
  - Removed duplicate `aria-label` attributes introduced during prior edits.

- **Database Backup Integrity in WAL Mode**:
  - Backup flow now uses SQLite `VACUUM INTO` for consistent snapshot behavior in WAL mode.
  - Eliminates risk of incomplete backups from plain file-copy approaches.

- **Workspace/Browser Tool Guidance Corrections**:
  - Browser `content` guidance strings updated to current parameter format.
  - Workspace `replaceLines` documentation clarified to match actual append behavior constraints.

- **Secure File Manager Size-Limit Enforcement**:
  - Hardened append-path size checks to prevent overflow-based file-size-limit bypass.
  - Added dedicated integration tests for append-limit scenarios.

### ⚡ Performance

- **Agent Chat Rendering Optimizations**:
  - Reduced unnecessary re-renders in `AgentMessageRenderer`, `AgentMessageBubble`, `ToolCallCompactItem`, and tool group components.
  - Memoization/comparator improvements lower UI work during streaming and tool-heavy sessions.

### 🔧 Internal

- Refactored agent LLM layer into focused submodules (`completion`, `prompt`, `response`, `types`) for maintainability.
- Refactored interactive browser server into cleaner modular structure.
- Removed obsolete lifecycle unit tests blocked by current crate test configuration and aligned lifecycle code cleanup.

### 📚 Docs

- Synced `README`, `CONTRIBUTING`, `agents.md`, and `CLAUDE.md` with current architecture and MCP behavior.
- Updated builtin-tool documentation and internal project notes to reduce docs drift.

## [0.5.8] - 2026-02-20

### 🐛 Fixes

- **Crash recovery: child agent session stuck as "paused" forever**: After a crash/restart, child agent sessions that were mid-execution would remain paused indefinitely and never resume. The awaitAgent tool now correctly resumes paused sessions through a new `POST /api/sessions/:id/resume` endpoint — no garbage messages are injected into the session history.
- **Session history list not reflecting live status**: The start-view session card list was not updating when a child session transitioned from `paused` to `busy` during crash recovery. The `AgentSessionListContext` now subscribes directly to `statusChanged` events and patches session status in-place without a full reload.
- **Garbage `[system] Resume after crash recovery.` message in child session**: The previous recovery mechanism injected a fake user message via `POST /messages`, polluting the session history and the LLM context. The new `/resume` endpoint triggers workflow continuation from existing messages without adding any new message.
- **"Session not found" on crash recovery kick**: Paused sessions at startup were not loaded into `active_sessions` memory, causing `/messages` POST to fail. The new `/resume` endpoint calls `resume_session` first (loads session into memory, recreates MCP proxy) before triggering the workflow.
- **Stale "Busy" in-memory status after crash recovery**: `recover_sessions` was inserting a stale `Busy` metadata snapshot for newly-recovered sessions, causing the next `start_workflow` call to see a busy session and silently queue rather than run.
- **Orphaned tool call spinners after crash**: Tool calls that were in-flight at crash time left the UI with permanently spinning tool result indicators. Crash recovery now injects tombstone error results for all unresolved tool calls, allowing the UI to unblock.

### 🔧 Internal

- New `POST /api/sessions/:id/resume` HTTP endpoint (`resume_session_workflow` handler) that loads a session into memory and resumes the LLM workflow from existing message history.
- `recover_sessions` explicitly sets `SessionStatus::Paused` on recovered metadata in both "new entry" and "already active" branches to prevent status snapshot race conditions.
- `last_message_is_ui_resource()` helper in `formatting.rs` distinguishes intentional UI-pause (awaiting user interaction) from crash-pause, preventing spurious resume kicks on sessions waiting for a UI resource response.
- Recovery tombstone messages tagged with `source: "recovery"` and filtered out from LLM context in `llm.rs` to avoid confusing the model.
- Added 7 TypeScript unit tests for `statusChanged` in-place patching and 9 Rust unit tests for `last_message_is_ui_resource` / `is_terminal_status` in `formatting.rs`.

## [0.5.7] - 2026-02-20

### 🐛 Fixes

- **Agent restart after cancel**: Fixed regression where cancelling an agent session permanently blocked subsequent messages. The workflow state machine now unconditionally resets the cancellation token on `start_workflow`, enabling cancel-then-continue as intended.
- **LLM abort error detection**: `isAbortError` utility now correctly identifies `DOMException`-based abort errors (not just `Error` subclasses), preventing false `error` status transitions after user-initiated cancellation in all browser environments.
- **Session status on abort**: Cancelled LLM requests now correctly transition session status to `idle` instead of `error`.

### 🔧 Internal

- Extracted shared `isAbortError()` utility from duplicated inline detection in `useLLMExecution` and `useLLMListener`.
- Added comprehensive cancel state machine test suite: 13 Rust integration tests (`tests/cancel_logic.rs`) + 20 TypeScript unit tests (`cancel.test.ts`).
- Rust cancel tests moved to integration test binary to avoid `STATUS_ENTRYPOINT_NOT_FOUND` DLL errors on Windows.

## [0.5.6] - 2026-02-20

### 🚀 Features

- **MCP Server Presets**:
  - New `MCPServerManagement` panel with curated preset catalog — one-click install for popular MCP servers (GitHub, Brave Search, Filesystem, etc.).
  - `MCPServerDialog` now supports preset selection, variable substitution UI, and environment variable definitions.
  - Backend `presets.rs` embeds `mcp-server.json` at compile time for zero-cost preset resolution.
  - Dedicated `/mcp-servers` route and sidebar nav entry.

- **Database Lifecycle Robustness**:
  - WAL journal mode enabled by default for improved write throughput and crash safety (`SqliteJournalMode::Wal`).
  - New `MigrationVerifier`: SHA-256 checksums over migration file list detect schema drift before startup.
  - New `BackupManager`: timestamped SQLite backups before migrations, auto-pruning oldest beyond 5 kept.
  - Structured `DatabaseError` enum replaces bare `String` errors across the lifecycle layer.
  - New `retry_with_backoff` / `retry_with_backoff_async` utilities power a quarantine-and-retry pattern in `run_with_sqlite_sync`.
  - New `SchemaVersionRecord` and schema info display for diagnostics.

- **Browser Tools — Playwright-Style Aliases**:
  - Short-name aliases now exposed: `goto`, `click`, `fill`, `type`, `content`, `back`, `forward`, `scroll`.
  - `content` is a smart router: no args → extract fresh page content; `page` arg → read from cache. Replaces both `extractWebContent` and `readWebContent`.
  - `create_rich_response`: all success paths now return live page title + URL as post-action verification.
  - `suggest_selectors`: element-not-found errors now auto-suggest up to 5 visible candidate selectors from the live DOM.
  - `all_tools()` now exposes the curated Playwright-style set only; legacy names remain routed for backward compatibility.

### 🐛 Fixes

- **`navigate_to_url` HTTP error routing**:
  - HTTP 403/401/404/5xx/timeout/Network Error responses now return targeted, action-specific guidance instead of silently passing through to `create_rich_response`.

- **`invalidate_cache` double write lock**:
  - Removed duplicate `state_cache.write()` call in `BrowserServer::invalidate_cache()`.

- **Browser tool description stale names**:
  - `click_element_tool` schema and `extract_web_content_tool` description corrected to reference `content` instead of old names.

## [0.5.5] - 2026-02-20

### 🚀 Features

- **MCP Server Presets**:
  - New `MCPServerManagement` panel with curated preset catalog — one-click install for popular MCP servers (GitHub, Brave Search, Filesystem, etc.).
  - `MCPServerDialog` now supports preset selection, variable substitution UI, and environment variable definitions.
  - Backend `presets.rs` embeds `mcp-server.json` at compile time for zero-cost preset resolution.
  - Dedicated `/mcp-servers` route and sidebar nav entry.

- **Database Lifecycle Robustness**:
  - WAL journal mode enabled by default for improved write throughput and crash safety (`SqliteJournalMode::Wal`).
  - New `MigrationVerifier`: SHA-256 checksums over migration file list detect schema drift before startup.
  - New `BackupManager`: timestamped SQLite backups before migrations, auto-pruning oldest beyond 5 kept.
  - Structured `DatabaseError` enum replaces bare `String` errors across the lifecycle layer.
  - New `retry_with_backoff` / `retry_with_backoff_async` utilities power a quarantine-and-retry pattern in `run_with_sqlite_sync`.
  - New `SchemaVersionRecord` and schema info display for diagnostics.

- **Browser Tools — Playwright-Style Aliases**:
  - Short-name aliases now exposed: `goto`, `click`, `fill`, `type`, `content`, `back`, `forward`, `scroll`.
  - `content` is a smart router: no args → extract fresh page content; `page` arg → read from cache. Replaces both `extractWebContent` and `readWebContent`.
  - `create_rich_response`: all success paths now return live page title + URL as post-action verification, so agents can confirm the action result without an extra call.
  - `suggest_selectors`: element-not-found errors now auto-suggest up to 5 visible candidate selectors from the live DOM.
  - `all_tools()` now exposes the curated Playwright-style set only; legacy names remain routed for backward compatibility.

### 🐛 Fixes

- **`navigate_to_url` HTTP error routing**:
  - HTTP 403/401/404/5xx/timeout/Network Error responses now return targeted, action-specific guidance (e.g., "Abandon this page" for 403) instead of silently passing through to `create_rich_response`.

- **`invalidate_cache` double write lock**:
  - Removed duplicate `state_cache.write()` call in `BrowserServer::invalidate_cache()` — redundant re-lock after first guard was dropped.

- **Browser tool description stale names**:
  - `click_element_tool` schema description still referenced `extractWebContent`; `extract_web_content_tool` description still referenced `readWebContent(page)`. Both corrected to `content`.

## [0.5.4] - 2026-02-19

### 🚀 Features

- **Session API Builtin Server**:
  - New builtin MCP server exposing tools for session management, swarm context queries, and assistant lookups directly from within agent sessions.
  - Added `GET /api/assistants/:id` endpoint for direct assistant retrieval by ID.

- **Agent Model Picker**:
  - Added `AgentModelPicker` component allowing users to select AI models and providers on a per-agent basis.

- **Enhanced Assistant Tools**:
  - Assistant tools now return detailed configuration including full tool schema and improved descriptions for better agent comprehension.

- **Database Backup**:
  - Implemented database backup functionality to safeguard user data.

- **Skills Download Prompt**:
  - `SkillsProvider` now detects missing skills and prompts the user to download them, improving the out-of-box agent experience.

### 🐛 Fixes

- **Windows Process Spawning**:
  - Restored `CREATE_NO_WINDOW` flag for stdio MCP server child-process spawning on Windows that was accidentally dropped in a prior merge.

- **Session API Tool Definitions**:
  - Removed duplicate `createSession` tool entry and restored the missing `getAssistant` tool definition and handler.

- **Skills Directory Resolution**:
  - Fixed inconsistent skills directory path resolution that caused skills to be found on some code paths but not others.

- **Migration & Cascade Deletes**:
  - Restored missing database migrations and cascade delete logic that were lost during a prior merge.

## [0.5.3] - 2026-02-17

### 🧠 Performance & Decision Quality (The "Orchestrator" Update)

- **Assistant Discovery Optimization**:
  - Enhanced `listAssistants`, `searchAssistant`, and `getAssistant` to include resolved **Skills** summaries.
  - Automatically resolves cryptic MCP server IDs into human-readable names (e.g., `google-search`) in the skill list, helping orchestrator agents (MasterMind) make faster decisions.

- **Intelligent Caching**:
  - Implemented a thread-safe, 30-second TTL cache for the server name map.
  - Eliminates redundant database hits during recursive assistant lookups while maintaining data freshness.

## [0.5.2] - 2026-02-16

### 🧘 Mental Clarity & Safety (The "Soul" Update)

- **Terminology Unification**:
  - Renamed `listExternalServers` → **`listServers`** (Simplified)
  - Renamed `listInternalTools` → **`listBuiltinTools`** (Clarified)
  - Merged `searchServer` into `listServers` (via `query` param)

- **Safety First (HashLine)**:
  - `editLineInFile` now **REQUIRES** `expected_hash` to prevent race conditions.
  - `searchLineInFile` now returns **content hash** (`a1b2`) along with text, enabling immediate safe edits.

- **Tool Diet**:
  - Hidden deprecated tools: `editFile`, `editFileMulti`, `searchServer`.
  - Minified tool descriptions to save context tokens.

## [0.5.1] - 2026-02-14

### 🐛 Fixes

- **Session Lineage Grouping (Start View)**:
  - Fixed a regression where clicking a session card's lineage focus could hide the parent while showing only child sessions.
  - Root cause: some sessions were created without normalized `lineageId`, causing lineage filtering mismatch between parent and child records.
  - Backend now normalizes lineage defaults during session creation (`lineageId` falls back to `sessionId` for roots, `parentSessionId` for children when missing).
  - Frontend session list normalization now backfills missing lineage/depth values for legacy records so parent/child lineage focus remains stable.

### ✅ Validation

- Re-ran API E2E flow via `scripts/test_api.py` after fix:
  - session create/resume/message polling,
  - parent-child lineage checks,
  - `maxDepth` limit behavior,
  - `maxFanout` limit behavior.
- Result: all scenarios passed.

## [0.5.0] - 2026-02-14

### 🚀 Features

- **Master Mind Autonomy Update**:
  - Updated default Master Mind doctrine to autonomy-first operation with explicit recovery duty.
  - Added automatic system prompt backfill/update for existing Master Mind assistant records.

- **Session Lineage Reliability**:
  - `createSession` and `createChildSession` now auto-resolve parent lineage from caller session context when omitted.
  - Added support for `parentSessionId: "current"` alias resolution.

- **Terminal Result Fidelity**:
  - `waitForSessionIdle` now returns full latest assistant text output by default (not short preview snippets).
  - Added optional `assistantMessageMaxChars` to cap output when needed.

### 🔧 Improvements

- **Timeout Baseline for Long-Running Tool Calls**:
  - Raised default MCP tool execution timeout from 60s to 180s across frontend defaults, UI fallback, and backend fallback config.

- **Documentation / Positioning Refresh**:
  - Rewrote README tone toward concise, product-confident messaging.
  - Added AI Soul manifesto documentation and docs index linkage.

### 🐛 Fixes

- Fixed parent-child session creation friction where child session creation could fail without explicit parent wiring in orchestration flows.
- Fixed wait-result truncation behavior that caused loss of final assistant detail in long reports.

## [0.4.25] - 2026-02-13

### 🐛 Fixes

- **Windows MCP Session Startup UX**:
  - Fixed console window flicker during agent session startup when spawning stdio MCP servers.
  - Applied `CREATE_NO_WINDOW` to Windows MCP child-process spawning path to prevent terminal pop-up/open-close flashes.

## [0.4.24] - 2026-02-13

### 🚀 Features

- **Agent Workflow Integrity & Cancellation Refactor**: Improved Agent V2 execution flow for interactive tool calls and cancellation handling.
  - **Message-Boundary Cancellation**: Cancel requests are now consumed at message boundaries during in-flight tool batches, improving consistency.
  - **Tool Result Ownership Validation**: Added expected/completed tool-call tracking per message to reject stale or duplicate tool results safely.
  - **API Semantics Alignment**: `agent_cancel_workflow` now reflects cancel-request semantics in responses.

- **Application Runtime Controls**:
  - Added app restart support integrated into settings.
  - Added HTTP server configuration options (port and exposure settings).

### 🐛 Fixes

- **Security Hardening for File Operations**:
  - Strengthened dropped-file registration/read flow with tighter validation.
  - Improved size and path checks to reduce TOCTOU and DoS risk in file access paths.

- **UI/UX Reliability Improvements**:
  - Fixed chat input tooltip and button interaction issues.
  - Improved chat rendering performance by reducing unnecessary re-renders in pending state handling.

### 🔧 Refactoring & Improvements

- **Lifecycle Modularization**: Refactored backend startup/lifecycle logic into clearer modules for maintainability and safer initialization flow.
- **Interactive Workspace Decomposition**: Split interactive workspace execution logic into focused modules (handlers/security/ui), improving clarity and testability.
- **Workspace Cleanup Robustness**: Added graceful shutdown behavior for background cleanup tasks to avoid leaked loops on teardown.

### ✅ Tests

- Added unit tests for:
  - tool-result classification (`accept / stale / duplicate`),
  - cancel strategy classification (`defer vs immediate`),
  - message-boundary cancel consumption predicates.

## [0.4.22] - 2026-02-07

### 🚀 Features

- **Skill Management Engine**: Added core infrastructure for Agent Skills
  - **New Editor Interface**: Added `SkillsEditor` component for managing agent capabilities
  - **Backend Commands**: Implemented `skill_management` and `skill_commands` in Rust backend
  - **Validation**: Added `skill_tests.rs` for ensuring skill integrity

- **Dependency Management**:
  - **Bun Support**: Added Bun lockfile and defined trusted dependencies (Commit `1a17ca4b`)

### 🔧 Refactoring

- **Project Cleanup**: Consolidating skill-related code and configuration

## [Unreleased]

### 2026-01-24

#### 🚀 Features

- **Draft Mode for Agent Sessions**: Introduced a streamlined session creation workflow
  - **New Draft Chat View** (`AgentDraftChatView`): Pre-session interface showing assistant profile with capabilities, tools, and configuration
  - **Atomic Session Creation**: New `agent_create_session_with_initial_message` Tauri command creates session and sends first message in single operation
  - **Instant Navigation**: Assistant selection from start view navigates to draft view (`/agent/draft?assistantId={id}`)
  - **Rich Assistant Profile**: Draft view displays assistant identity (name, description), capability badges (built-in tools, MCP servers), and active model/provider configuration
  - **Seamless Transition**: On first message submission, session is atomically created and user is redirected to full persistent view (`/agent/{sessionId}`)
  - **User Experience**: Eliminates empty session creation, allows users to review assistant setup before committing

- **Built-in Server Metadata System**: Standardized metadata retrieval across all built-in MCP servers
  - **Static Metadata Functions**: All built-in servers (`AssistantServer`, `KnowledgeServer`, `PlanningServer`, `PlaybookServer`, `BrowserServer`, `WorkspaceServer`, `ContentStoreServer`) now implement `metadata_static()` method
  - **Centralized Registry**: `list_available_builtin_server_definitions()` returns static metadata for all possible builtin servers (used in UI for assistant configuration)
  - **UI Integration**: Draft view fetches server metadata to display service names, descriptions, and icons dynamically
  - **Consistent Interface**: All metadata includes `displayName`, `description`, and optional `icon` field

#### 🔧 Refactoring & Improvements

- **Assistant Name in Message Bubbles**: Agent chat messages now display actual assistant name instead of generic "Agent" label
  - **Context Propagation**: `getAssistantNameForMessage` callback uses `session?.assistant?.name` from session context
  - **Streaming Messages**: Assistant name displayed during agent workflow execution
  - **Personalization**: Improves user experience by showing configured assistant identity

- **Unified Server Name Aliases**: Improved server name resolution in `list_builtin_tools_for()`
  - **Multiple Aliases**: Added support for alternative server names (`assistant` OR `assistant_manager`, `content_store` OR `contentstore`)
  - **Backwards Compatibility**: Ensures tools are correctly resolved regardless of server name variant used

- **Tool Metadata Consolidation**: Refactored server metadata to use static methods instead of hardcoded values
  - **DRY Principle**: Eliminated duplicate metadata definitions across `list_available_builtin_server_definitions()`
  - **Single Source of Truth**: Metadata now sourced from server implementations (e.g., `knowledge::KnowledgeServer::metadata_static()`)
  - **Maintainability**: Server changes automatically reflected in metadata API without manual updates

#### 🐛 Bug Fixes

- **Duplicate Command Registration**: Removed duplicate `get_setting` and `delete_setting` entries in `lib.rs` invoke handler list
  - **Issue**: Commands were registered twice causing potential handler conflicts
  - **Fix**: Cleaned up duplicate registrations in main invoke handler array

#### 📝 Documentation

- **Version Bump**: Updated Cargo.lock to reflect version 0.4.4

### 2026-01-18

#### 🔧 Refactoring & Improvements

- **Built-in Tool Best Practices Alignment**: Comprehensive refactor of `assistant`, `mcp_manager`, and `bootstrap` modules to align with project best practices.
  - **Assistant Module**:
    - Split monolithic `mod.rs` into feature-based files: `queries.rs` (read-only) and `operations.rs` (write-only).
    - Enhanced tool descriptions with **⚠️ CRITICAL WORKFLOW** instructions to guide AI agents (e.g., checking for duplicates before creation).
    - Integrated `SuccessHint` for consistent success responses with "💡 Next Steps".
    - Improved error guidance using the centralized `error_guidance` system.
  - **MCP Manager Module**:
    - Reorganized into `queries.rs` and `operations.rs`.
    - Restored missing `deleteServer` and `updateServer` tools.
    - Updated `listServers` with improved descriptions and workflows.
    - Fixed `SeaORM` usage (removed redundant references) and improved query reliability.
  - **Bootstrap Module**:
    - Enforced strict canonical naming by removing legacy `builtin_bootstrap__*` aliases.
    - Confirmed semantic alignment of 'bootstrap' naming with module responsibilities (environment setup vs. workspace execution).

#### 🚀 Features

- **App Wizard Initialization**: Improved automatic initialization of default assistants in `assistant_init.rs`.
  - Ensures default assistants (`Bootstrap`, `Libr`) are created with correct configurations on first launch.
  - Standardized configuration handling for built-in service aliases.

#### 🐛 Fixed & Maintenance

- **Lint & Compilation**: Resolved numerous Rust compiler warnings (unused imports, case naming) and lint errors.
- **Test Stability**: Temporarily disabled failing legacy tests in `content_store/test_recent_uploads.rs` to maintain a green validation pipeline while investigating system visibility issues.
- **Validation**: Full `pnpm refactor:validate` pipeline passed, ensuring code quality across frontend and backend.

### 2026-01-16

#### 🔧 Refactoring

- **File Operation Tool Naming Refactor**: Semantic rename of workspace file operation tools for clarity
  - **Tool Names**: `writeFile` → `createFile` and `replaceStringInFile` → `editFile`
  - **Phase 1 - Function Renames**: Renamed all internal functions and dispatcher routing
    - `create_write_file_tool()` → `create_create_file_tool()`
    - `create_replace_string_in_file_tool()` → `create_edit_file_tool()`
    - `handle_write_file()` → `handle_create_file()`
    - `handle_replace_string_in_file()` → `handle_edit_file()`
  - **Phase 2 - Documentation Updates**: Updated all tool descriptions, guidance messages, and error hints
    - Tool descriptions now reference correct tool names
    - Error guidance messages updated (10+ locations)
    - Error hints now catch old tool names and suggest new ones
  - **Semantic Clarity**: New names better describe tool purpose:
    - `createFile`: Create new files or overwrite existing files
    - `editFile`: Make targeted edits to existing file content using exact string matching
  - **User Experience**: Error messages improved when users try old tool names
    - `"Did you mean 'createFile'? Please use the exact tool name 'createFile' to create or write files."`
    - `"Did you mean 'editFile'? Please use the exact tool name 'editFile' to edit file content."`
  - **Breaking Change**: Old tool names no longer recognized (agents must use new names)
  - **Implementation Details**:
    - Total edits: 17 across 3 files (tools/file_tools.rs, file_operations.rs, mod.rs)
    - Compilation: ✅ All checks pass (cargo check, pnpm lint, pnpm build, dead-code)
    - Validation: ✅ Full refactor:validate pipeline passed
  - **Documentation**: Comprehensive implementation guides created in `docs/guides/`
    - `phase1-completion-summary.md`: Phase 1 technical changes
    - `phase2-completion-summary.md`: Phase 2 documentation updates

### 2026-01-12

#### 🚀 Features

- **Settings Backend Migration to Rust/SeaORM**: Completed Phase 2 migration of settings persistence layer
  - **New Settings Table**: Added `settings` table with key-value JSON storage (replaces IndexedDB `objects` table)
  - **SeaORM Entity**: Created `settings` entity with CRUD operations and timestamps
  - **Rust Service Layer**: Implemented `RustSettingsService` in TypeScript calling Tauri commands
  - **Singleton Pattern**: Exported `settingsService` singleton for consistent access
  - **Migration Path**: Settings data automatically migrated from IndexedDB to SQLite on first load
  - **Type Safety**: Full TypeScript support with existing `Settings` interface
  - **Test Coverage**: Added comprehensive CRUD tests in `seaorm_migration_verification.rs`

- **CRUD Commands for Content Store Entities**: Added complete Tauri command layer for content store management
  - **Assistant CRUD**: `create_assistant`, `update_assistant`, `delete_assistant`, `list_assistants`, `get_assistant`
  - **MCP Server Config CRUD**: `create_mcp_server_config`, `update_mcp_server_config`, `delete_mcp_server_config`, `list_mcp_server_configs`
  - **Playbook CRUD**: `create_playbook`, `update_playbook`, `delete_playbook`, `list_playbooks` (with optional session filtering)
  - **Settings CRUD**: `set_setting`, `get_setting`, `delete_setting`, `list_settings` (upsert-style key-value operations)
  - **Rust Service Implementations**: Added `RustAssistantService` replacing IndexedDB-based `LocalAssistantService`
  - **Unified Registration**: All CRUD commands registered in `lib.rs` for frontend consumption

- **Default Assistants Initialization**: Automatic creation of default assistants on first launch
  - **Bootstrap Assistant**: Helps users set up their environment with platform detection and tool installation guidance
  - **Libr Assistant**: General-purpose knowledge and automation agent with planning and memory capabilities
  - **Service Layer**: New `assistant_init` module ensures default assistants exist after migrations
  - **Deletion Protection**: Default assistants marked with `deletionProtected: true` flag

#### 🔧 Enhancements

- **Settings Page Scroll Fix**: Fixed overflow clipping issue preventing users from viewing bottom content
  - **Root Container Update**: Added `overflow-y-auto` to route container in `App.tsx`
  - **Tab Content Accessibility**: Users can now scroll through all settings tabs regardless of height
  - **Layout Preservation**: Maintains flex layout with proper scrolling behavior

- **Assistant Service Refactoring**: Migrated assistant persistence from IndexedDB to Rust backend
  - **Removed BM25 Index**: Eliminated client-side search index (now handled server-side in future)
  - **Simplified Search**: Client-side filtering for now, server-side implementation planned
  - **Service Composition**: `AssistantService` now wraps `RustAssistantService` for local ops
  - **Remote Sync**: Maintains Agent Hub remote sync capabilities
  - **Backward Compatibility**: Exported `assistantService` singleton for existing consumers

- **Playbook Type Enhancement**: Added optional `id` field to `Playbook` interface
  - **Creation Flexibility**: ID can be omitted for creation (backend auto-generates)
  - **Update Support**: ID required for updates and deletes
  - **Type Safety**: TypeScript enforces proper ID usage in CRUD operations

#### 🐛 Fixed

- **AgentChatContext Test Fix**: Updated test expectations to account for `SettingsProvider` initialization
  - **Settings Initialization**: `SettingsProvider` now calls `list_settings` on mount
  - **Mock Validation**: Tests updated to verify no agent commands called during inactive session
  - **Retry Test**: Adjusted expected invoke count to include settings initialization call

#### 📦 Dependencies

- **SeaORM Migration**: Added `m20260112_000001_create_settings_table` migration
- **Chrono**: Utilized for timestamp generation in CRUD operations
- **UUID**: Used for auto-generating assistant IDs in default initialization

---

### 0.4.0 Milestone (Previous Changes)

#### 🚀 Features

- **Primary Isolated Shell Tools**: Introduced new lightweight shell execution tools for faster standard operations
  - **New Primary Tools**: `runShell` (Unix) and `runPowerShell` (Windows) for synchronous, isolated execution
  - **Workspace Anchoring**: Primary tools always execute from workspace root for predictability
  - **Performance**: Eliminates persistent shell overhead for 90% of common commands (ls, cat, grep)
  - **Clear Separation**: Renamed `executeShell` to `runInPersistentShell` and `executeWindowsCmd` to `runInPersistentPowerShell`
  - **Usage Guidance**: Updated service context to guide agents toward correct tool selection

- **Gemini Granular Token Metrics**: Enhanced usage tracking for Gemini models
  - **Usage Breakdown**: Tracks prompt, completion, and total tokens independently
  - **Cache Visibility**: Reports `cachedContentTokenCount` for context caching optimization
  - **Thinking Tokens**: Tracks `thoughtsTokenCount` for reasoning models

- **Rust Agent Core**: Implement initial Rust-based agent core (`thronglet`) with Tauri integration, new frontend components, and packaging configurations.
- **Multi-Vendor Prefill Performance Tracking**: Added Time-To-First-Token (TTFT) measurement across all AI service providers for consistent prefill performance monitoring.
  - **Native Metrics**: Ollama provides `promptEvalDuration`, Anthropic provides cache hit metrics
  - **Client-Side TTFT**: OpenAI, Groq, Fireworks, Cerebras, and Gemini now measure TTFT using `performance.now()`
  - **Unified Interface**: All providers report prefill timing via `TokenUsage.details.timeToFirstToken` or `promptEvalDuration`
  - **UI Display**: Token metrics badge shows prefill timing in tooltip (hover over input token count)
  - **Usage Merging**: LLMServiceContext properly merges TTFT with final token usage data
- **User-Configurable Metric Display Settings**: New Display settings tab allowing users to customize how token metrics are shown:
  - **Metric Display Mode**: Choose between inline display (shown in message) or tooltip display (hover to see)
  - **Prefill Performance Format**: Display prefill performance as Time to First Token (e.g., 245ms) or as Tokens Per Second (e.g., 520 tok/s)
  - **Show Token Speed**: Toggle generation speed display (tokens per second)
  - **Compact Metrics**: Enable compact display format for token metrics
  - **Settings Persistence**: All display preferences are saved to IndexedDB and persist across sessions

### 🔧 Enhancements

- **Content Store Context Enhancement**: Improved agent visibility of uploaded files through enhanced service context
  - **Recent Uploads Tracking**: System prompt now displays last 10 uploaded files with IDs and metadata
  - **Direct File Access**: Agents can immediately use `contentId` without calling `list()`
  - **Smart Truncation Logic**: Fixed misleading truncation messages - now distinguishes between preview truncation and file-end detection
  - **Enhanced Error Messages**: Out-of-bounds errors now include actual file size and valid range suggestions
  - **Content ID Normalization**: Auto-adds `content_` prefix to IDs for flexible usage
  - **Token Efficiency**: Service context limited to 10 most recent files (~200 tokens max)
  - **Session Isolation**: Recent uploads reset on session switch
  - **New Tests**: Added comprehensive test suite for upload tracking and service context generation

- **Workspace Output Visibility Fix**: Enhanced LLM visibility of process execution output
  - **readProcessOutput**: Now includes actual stdout/stderr content in text field (not just "Read N lines" summary)
  - **pollProcess with tail**: Appends last N lines of output directly to status message for quick inspection
  - **Test Coverage**: Added `test_output_visibility.rs` to verify LLM can see command output

- **File Reading Line Number Display**: Improved readability of file content with line number formatting
  - **Line Numbers**: All file reads now include `Line N:` prefix for each line
  - **Empty Line Collapsing**: Multiple consecutive empty lines collapsed to `<Empty Lines N-M>` placeholder
  - **Token Efficiency**: Reduces context size for files with many blank lines
  - **Test Coverage**: Added comprehensive tests for line formatting logic

- **Persistent Shell PATH Fix**: Fixed missing `~/.local/bin` in non-interactive bash sessions
  - **Auto PATH Extension**: Automatically adds `~/.local/bin` to PATH if missing (critical for pip-installed binaries)
  - **Exit Code Capture**: Fixed race condition in bash sentinel - now captures `$?` before echoing sentinel
  - **No User Impact**: Change is transparent to users, tools work as expected without manual PATH setup

### 🐛 Fixed

- **Token Estimation Robustness**: Fixed WASM crash in `tiktoken` for non-OpenAI models
  - **Try-Catch Wrapper**: Added robust error handling around WASM calls in `estimateTextTokens`
  - **Heuristic Fallback**: Implemented character-based fallback (approx 4 chars/token) when tokenizer fails
  - **Ollama Stability**: Prevents `RuntimeError: Unreachable code` when using Qwen/Llama via Ollama

- **Knowledge Tool Fixes (Critical)**:
  - **Schema Mismatch Resolved**: Fixed a critical bug where `searchKnowledge` failed due to a missing `Source` column in the database schema. Added a migration to introduce the `source` column to the `knowledge` table.
  - **Search Snippets**: Updated `searchKnowledge` to return relevant text snippets using SQLite FTS5 `snippet()` function (or `substr` fallback), providing immediate context in search results.
  - **Source Filtering**: Added support for filtering knowledge search results by `source` URL.
  - **Data Integrity**: Updated `saveKnowledge`, `readKnowledge`, and `listKnowledge` to correctly handle and persist the `source` field.

- **LLM Service Empty Response Handling**: Relaxed validation to allow responses with usage but no content
  - **Reasoning Model Support**: Ollama reasoning models may return empty content but non-zero completion tokens (valid thinking-only responses)
  - **Diagnostic Logging**: Added detailed logging before throwing empty response error for better debugging
  - **Graceful Degradation**: Logs warning but allows proceeding when usage indicates model did work

- **Settings Page Debounce Extraction**: Extracted debounce logic into reusable `useDebounce` hook
  - **Code Reuse**: Consolidated debounce/throttle patterns into single hook
  - **Type Safety**: Full TypeScript generic support with proper parameter inference
  - **API Consistency**: Provides `debounced`, `cancel`, and `flush` methods
  - **Testing Ready**: Hook can be easily unit tested

- **Ollama Chunk Processing Diagnostics**: Added comprehensive logging for empty chunks
  - **Missing Field Detection**: Warns when chunk has unexpected structure (keys but no known fields)
  - **Raw Chunk Logging**: Logs full JSON for unrecognized chunks to aid debugging
  - **Model-Specific Handling**: Better support for edge cases in different Ollama model outputs

- **Tool Argument Validation**: Enhanced type safety for tool call argument parsing
  - **Zod Schema Validation**: Tool arguments now validated to ensure they're objects (not arrays/primitives)
  - **Graceful Fallback**: Invalid structures wrapped in `{ value: ... }` instead of throwing errors
  - **Error Logging**: Detailed debug logs for parse failures with full context
  - **Type Guards**: Added `isMCPErrorContent` guard for safe error checking

- **Database Singleton Reset**: Fixed potential memory leak in `LocalDatabase`
  - **Null Type Safety**: Changed `instance` from `LocalDatabase` to `LocalDatabase | null` for proper nullable handling
  - **Type-Safe Reset**: `resetInstance()` no longer requires `as unknown as` cast
  - **Test Isolation**: Prevents test pollution by properly cleaning up singleton

### 🐛 Fixed

- **Knowledge Tool Fixes (Critical)**:
  - **Schema Mismatch Resolved**: Fixed a critical bug where `searchKnowledge` failed due to a missing `Source` column in the database schema. Added a migration to introduce the `source` column to the `knowledge` table.
  - **Search Snippets**: Updated `searchKnowledge` to return relevant text snippets using SQLite FTS5 `snippet()` function (or `substr` fallback), providing immediate context in search results.
  - **Source Filtering**: Added support for filtering knowledge search results by `source` URL.
  - **Data Integrity**: Updated `saveKnowledge`, `readKnowledge`, and `listKnowledge` to correctly handle and persist the `source` field.

### 🔧 Refactoring

- **MCP Type System Complete Cleanup** (Phase 2): Removed all legacy MCP configuration types and conversion code. The codebase now uses a single, clean `MCPServerConfig` type throughout.
  - **Backend (Rust)**:
    - Removed `LegacyMCPServerConfig` struct and all conversion logic (~140 lines)
    - Removed `MCPServerConfigWrapper` enum (no longer needed)
    - Simplified `list_tools_from_config` to parse `MCPServerConfig` directly
    - Replaced 6 legacy conversion tests with 3 clean serialization tests
  - **Frontend (TypeScript)**:
    - Removed `LegacyMCPServerConfig` interface completely
    - Removed utility functions: `isModernConfig()`, `convertLegacyToModern()`
    - Updated `MCPConfig` to use only `MCPServerConfig` (no union types)
    - Updated imports in `chat.ts`, `server-config.ts`
  - **Validation**:
    - ✅ TypeScript compilation: 0 errors
    - ✅ Rust tests: 3/3 passed (stdio, http, oauth serialization)
    - ✅ All existing modern configs work without changes
  - **Note**: Early dev stage - no migration needed. Phase 1 (V2 suffix removal) completed earlier.

## [0.3.43] - 2025-12-23

### ✨ Added

#### AI Service & Tools

- **Advanced Settings**: Added advanced settings support for AI service and tool processor.
- **Smart Tools**: Integrated `listInteractableSmartTool` for improved semantic filtering in browser interactions.
- **Workspace Tools**: Added built-in workspace tools for file management, code execution, and data export.
- **Settings**: Implemented factory reset functionality.

#### Workflow & Management

- **Todo Nesting**: Implemented 1-level nesting for todos with parentId and subtasks support.
- **Enhanced Responses**: Enhanced scratchpad and knowledge management responses with detailed formatting.

### 🛠️ Improvements

- **Reliability**: Implemented error circuit breaker logic in tool processor.
- **Browser**: Enhanced `clickElement` to wait for page load after navigation.
- **Performance**: Implemented strict line-based chunking in `content-store`.

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
