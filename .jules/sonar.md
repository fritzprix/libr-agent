# Sonar: Coverage Expansion Log

This journal tracks the relentless expansion of test coverage by "Sonar".

Format: `## YYYY-MM-DD - [Module/File] **Coverage Expanded:** [Tested Target] **Assertions:** [What was verified]`

---

## 2026-02-21 - src/lib/date-utils.ts **Coverage Expanded:** formatRelativeTime, formatSessionTimestamp **Assertions:** Verified relative time formatting thresholds, future dates, and session timestamp structure.

## 2026-02-21 - src/lib/retry-utils.ts **Coverage Expanded:** sleep, withTimeout, withRetry, withRetryResult **Assertions:** Verified async delays, timeout rejection, retry success/failure paths, and exponential backoff timing.

## 2026-02-21 - src/lib/tool-call-utils.ts, src/lib/utils.ts **Coverage Expanded:** Error handling, tool parsing, string formatting, throttling **Assertions:** Verified hasToolCallError, parseToolArguments JSON handling, formatExecutionTime precision, and throttlePromise argument preservation. **Bug Fixed:** throttlePromise memory leak - now tracks all pending resolvers in array and resolves them all with final result, preventing promise hang. Test improvements: Added promise resolution verification and factory function for type-safe Message test fixtures.

## 2026-02-23 - src/lib/services/assistant-service.ts **Coverage Expanded:** AssistantService, RemoteAssistantService **Assertions:** Verified hybrid local/remote sync logic, fallback mechanisms for CRUD operations (getAll, getById, save, delete), pagination handling, and search query propagation.

## 2026-02-24 - src/lib/llm-config-manager.ts **Coverage Expanded:** LLMConfigManager (all methods) **Assertions:** Verified provider/model retrieval, getLangchainModelId mapping, filtering (tools, reasoning, cost), and recommendModel logic. **Bug Fixed:** Added missing `gemini` -> `google-genai` mapping in `getLangchainModelId` to prevent runtime errors.
