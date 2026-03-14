# Sonar: Coverage Expansion Log

This journal tracks the relentless expansion of test coverage by "Sonar".

Format: `## YYYY-MM-DD - [Module/File] **Coverage Expanded:** [Tested Target] **Assertions:** [What was verified]`

---

## 2026-02-21 - src/lib/date-utils.ts **Coverage Expanded:** formatRelativeTime, formatSessionTimestamp **Assertions:** Verified relative time formatting thresholds, future dates, and session timestamp structure.

## 2026-02-21 - src/lib/retry-utils.ts **Coverage Expanded:** sleep, withTimeout, withRetry, withRetryResult **Assertions:** Verified async delays, timeout rejection, retry success/failure paths, and exponential backoff timing.

## 2026-02-21 - src/lib/tool-call-utils.ts, src/lib/utils.ts **Coverage Expanded:** Error handling, tool parsing, string formatting, throttling **Assertions:** Verified hasToolCallError, parseToolArguments JSON handling, formatExecutionTime precision, and throttlePromise argument preservation. **Bug Fixed:** throttlePromise memory leak - now tracks all pending resolvers in array and resolves them all with final result, preventing promise hang. Test improvements: Added promise resolution verification and factory function for type-safe Message test fixtures.

## 2026-02-23 - src/lib/services/assistant-service.ts **Coverage Expanded:** AssistantService, RemoteAssistantService **Assertions:** Verified hybrid local/remote sync logic, fallback mechanisms for CRUD operations (getAll, getById, save, delete), pagination handling, and search query propagation.

## 2026-02-24 - src/lib/llm-config-manager.ts **Coverage Expanded:** LLMConfigManager (all methods) **Assertions:** Verified provider/model retrieval, getLangchainModelId mapping, filtering (tools, reasoning, cost), and recommendModel logic. **Bug Fixed:** Added missing `gemini` -> `google-genai` mapping in `getLangchainModelId` to prevent runtime errors.

## 2026-02-25 - src/features/agent/hooks/useAgentFileAttachment.ts **Coverage Expanded:** useAgentFileAttachment **Assertions:** Verified MIME type detection, dropped file registration/reading, file size validation, and file input handling.

## 2026-02-25 - src/lib/message-preprocessor.ts **Coverage Expanded:** prepareMessageForLLM, prepareMessagesForLLM **Assertions:** Verified attachment handling (Content Store, Workspace, Metadata only), error resilience (returns original message), and logging statistics.

## 2026-02-25 - src/lib/mcp/utils/type-guards.ts, src/lib/mcp/utils/service-info.ts, src/lib/mcp/protocol/content.ts **Coverage Expanded:** MCP Type Guards & Utilities **Assertions:** Verified isMCPSuccess, isMCPError, isValidMCPResult, extractStructuredContent, hasServiceInfo, and isMCPErrorContent. **Bug Fixed:** `hasServiceInfo` now correctly returns boolean false instead of null for null inputs.

## 2026-02-25 - src/lib/ai-service/sanitizer.ts, src/lib/ai-service/utils.ts **Coverage Expanded:** AI Service Utilities & Sanitizer **Assertions:** Verified message sanitization (tool_calls, thinking), token calculation, usage formatting, and multimodal content processing.

## 2026-02-28 - src/features/agent/lib/chat-utils.ts **Coverage Expanded:** computeDisplayContent **Assertions:** Verified content array parsing, grouped tools restructuring, and original fallback filtering.

## 2026-03-01 - src/lib/mcp/schema/builders.ts, src/hooks/use-settings.ts **Coverage Expanded:** JSON Schema Builders, useSettings Hook **Assertions:** Verified robust typed schema object creation, expected error handling outside context providers, and data retrieval inside providers.

## 2026-03-03 - src/hooks/useDebounce.ts, src/hooks/use-agent-tools.ts **Coverage Expanded:** Debounce Timing, Agent Tool Loading Hook **Assertions:** Verified accurate timer delays/cancellations using fake timers and verified API fetching, validation, error handling, and component unmount safety.

## 2026-03-03 - src/lib/retry-utils.ts **Coverage Expanded:** withRetry, withRetryResult **Assertions:** Verified timeout behavior per attempt, correct exact base delay without exponential backoff, loop termination edge cases, and jitter bounds scaling accurately via fake timers.

## 2026-03-03 - src/lib/session-utils.ts **Coverage Expanded:** filterSessions **Assertions:** Verified graceful handling of AgentSession objects missing an assistant field during query filtering.

## 2026-03-03 - src/lib/tool-call-utils.ts **Coverage Expanded:** isBuiltinTool, parseBuiltinToolName, parseToolArguments **Assertions:** Verified boundary conditions for extracting tool prefixes and ensured robust recovery (returning '{ raw: string }') when JSON parsing throws non-Error objects.

## 2026-03-03 - src/test/setup.ts **Coverage Expanded:** Global Test Architecture **Assertions:** Initialized a dummy `i18next` instance in global setup to eradicate `NO_I18NEXT_INSTANCE` console warning spew during unit tests. Preserves `useTranslation` hook signature while eliminating React warnings. Tests correctly fallback to English JSON keys.

## 2026-03-07 - src/lib/backend/utils.ts, src/lib/backend/agent-commands.ts, src/lib/backend/builtin-tools.ts, src/lib/backend/browser.ts, src/lib/backend/file-operations.ts **Coverage Expanded:** Tauri command wrappers **Assertions:** Verified safeInvoke parameters and return values.

## 2026-03-08 - src/models/validation.ts **Coverage Expanded:** isValidMessage, parseAssistant **Assertions:** Verified boundary checks for Message hydration and validation (id, sessionId, threadId, role presence/type validation, content array verification). Added coverage for missing branches where `isValidMessage` handles nullish checks correctly, and `parseAssistant` handles invalid JSON stringified config or null config gracefully.

## 2026-03-09 - src/lib/backend/skills.ts **Coverage Expanded:** Backend Skills Wrapper **Assertions:** Verified safeInvoke args and correct payload forwarding

## 2026-03-09 - src/lib/backend/scheduled-tasks.ts **Coverage Expanded:** Backend Scheduled Tasks Wrapper **Assertions:** Verified safeInvoke args and correct payload forwarding

## 2026-03-09 - src/lib/backend/workspace.ts **Coverage Expanded:** Backend Workspace Wrapper **Assertions:** Verified safeInvoke args and correct payload forwarding

## 2026-03-09 - src/lib/backend/playbooks.ts **Coverage Expanded:** Backend Playbooks Wrapper **Assertions:** Verified safeInvoke args, correct payload forwarding, JSON deserialization robustness, and pagination logic

## 2026-03-11 - src/lib/backend/mcp-server.ts **Coverage Expanded:** MCP Server Backend Interface API **Assertions:** Verified accurate routing and argument payloads for all backend commands (callTool, hasOAuthToken, getOAuthToken, revokeOAuthToken, sampleFromModel, validateToolSchema) with 100% statement and branch coverage.

## 2026-03-12 - src/lib/date-utils.ts **Coverage Expanded:** formatSessionTimestamp relative fallback logic **Assertions:** Verified absolute fallback when relative format is empty

## 2026-03-13 - src/lib/backend/messages.ts **Coverage Expanded:** Message Management and Search **Assertions:** Validated input checking, type mapping, and SafeInvoke bridging
