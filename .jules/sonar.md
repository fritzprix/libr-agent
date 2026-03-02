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
## 2026-03-02 - src/lib/mcp/schema/builders.ts, src/lib/mcp/utils/type-guards.ts, src/lib/mcp/utils/service-info.ts, src/lib/mcp/protocol/content.ts **Coverage Expanded:** MCP Types & Utilities **Assertions:** Added missing tests for schema builders, extracted coverage metric fixes.
