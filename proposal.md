# Debugging Request: readContent -> tool result loss (stored as [{"type":"text"}])

## Purpose

This document summarizes an observed bug where `readContent` tool results arrive intact through the WebMCP worker/proxy/provider stack, but the final tool-result message stored in session history (and sent to the LLM) contains the string `'[{"type":"text"}]'` instead of the original text payload. The goal is to give the development team a concise, reproducible report and step-by-step debugging plan so the root cause can be diagnosed and fixed quickly.

## Priority

- High: This breaks UX and LLM context when reading attached files.

## Affected components / files

- Frontend MCP stack (tool execution path)
  - `src/lib/web-mcp/mcp-worker.ts`
  - `src/lib/web-mcp/mcp-proxy.ts`
  - `src/features/tools/WebMCPToolProvider.tsx`
- Chat and tool execution orchestration
  - `src/context/ChatContext.tsx` (ToolCaller & submit path)
  - `src/hooks/use-ai-service.ts`
- AI provider layer
  - `src/lib/ai-service/gemini.ts`
- Message preprocessor (attachments)
  - `src/lib/message-preprocessor.ts`

## Observed behavior (summary)

1. User triggers a `readContent` tool call for an attached file.
2. Logs show the worker/proxy/provider path receives and logs the full text (large markdown) as `result.content`.
3. `WebMCPToolProvider` logs the same full text response.
4. When `ToolCaller` (in `ChatContext`) converts the MCP response to a `Message` and persists it, the saved message `content` is a string equal to `'[{"type":"text"}]'` (or similarly minimal JSON), i.e. the actual text is missing and replaced by a compact structural array string.
5. As a result, the LLM input and Chat history contain no readable file content; downstream processing reports inability to read file content.

## Key evidence (log excerpts)

- Worker/proxy log showing full content (truncated here):

```
[WebMCPProxy] callTool response from worker {"serverName":"content-store","toolName":"readContent","result":{"content":"# **MCP File Attachment System Development Plan (Python)**\n\n## **Overview**\n\n파일 첨부 시스템을 MCP ...","lineRange":[1,253]}}
```

- Provider log confirming the provider sees the same content:

```
[WebMCPToolProvider] WebMCPToolProvider executeTool result { "serviceId":"content-store", "result": { "content": "# **MCP File..." } }
```

- ChatContext submit log shows `messageToAdd` with `content` set to the compact JSON string:

```
[ChatContext] submit  { messageToAdd: [ { id: 'c58k...', role: 'tool', content: '[{"type":"text"}]', tool_call_id: 'tool_pkx...' } ] }
```

## Investigation findings

- The WebMCP worker, proxy, and provider components log the full textual content — the worker/proxy/provider chain appears to be functioning correctly.
- The `ToolCaller` implementation in `ChatContext.tsx` converts the MCP response into a `Message` for storage.
  - Code of interest (simplified):

```ts
content: typeof mcpResponse.result?.content === 'string'
	? mcpResponse.result.content
	: JSON.stringify(mcpResponse.result?.content ?? ''),
```

- This means: if `mcpResponse.result.content` is not a string (for example, an MCPContent[]), the code will JSON.stringify that value and persist the JSON text.
- The persisted string `'[{"type":"text"}]'` suggests that `mcpResponse.result.content` is an array of MCPContent items where at least one element has type 'text' but its `text` field may be empty or missing by the time it reaches `ToolCaller`.
- Two plausible root causes:
  1.  `executeToolCall` (UnifiedMCP layer) returns `result.content` as an array (MCPContent[]). That array may contain text items whose `text` fields are empty/not present at this stage.
  2.  Provider/proxy returned structured MCPContent[] intentionally while earlier logs showing the plain text are from a different field (for example `result.content` vs `result`), causing confusion about which value is persisted.

## Hypothesis

- The immediate problem is in the `ToolCaller` persistence logic: it fails to normalize MCPContent[] into a readable string. Even if the provider returns MCPContent[] (which is a valid shape), the `ToolCaller` should prefer extracted text from that array rather than JSON.stringify the structure.

## Recommended immediate actions (fast diagnostics)

1. Add a non-destructive debug log in `ToolCaller` right after receiving `mcpResponse` to print the raw response structure (including `result` and `result.content`) so we can confirm the exact runtime shape.
   - File: `src/context/ChatContext.tsx` inside the tool execution loop.
   - Example log: `logger.debug('Raw mcpResponse for tool', { toolCallId: toolCall.id, mcpResponse });`

2. Inspect `executeToolCall` / unified MCP wrapper to confirm the canonical return shape for `result.content` (string vs MCPContent[]). Ensure the type is documented and consistent.

## Quick mitigation (recommended patch)

Modify `ToolCaller` persistence logic to normalize `mcpResponse.result.content` into a readable string before saving. This is non-destructive and preserves current behavior when only a string is returned.

Patch sketch (safe, minimal):

```ts
const rawContent = mcpResponse.result?.content;
let contentString: string;

if (typeof rawContent === 'string') {
  contentString = rawContent;
} else if (Array.isArray(rawContent)) {
  // MCPContent[] -> prefer text parts
  const textParts = rawContent
    .filter((it: any) => it && it.type === 'text' && 'text' in it)
    .map((it: any) => it.text)
    .filter(Boolean as any);
  contentString = textParts.length
    ? textParts.join('\n')
    : JSON.stringify(rawContent);
} else {
  contentString = JSON.stringify(rawContent ?? '');
}

// then persist content: contentString
```

This ensures readable text is stored when available, and falls back to JSON only when there is no text payload.

## Longer-term fixes

- Standardize the `executeToolCall` / unified MCP return shape and update TypeScript types:
  - `result.content: string | MCPContent[]`
  - Add utility `normalizeMCPContentToString(content: string | MCPContent[]): string` in a shared utils module.
- Add unit tests for tool processing and message persistence including cases where provider returns `MCPContent[]`.

## Tests to add

- Unit: `normalizeMCPContentToString` should convert:
  - string -> same string
  - MCPContent[] with text parts -> concatenated text
  - MCPContent[] without text parts -> JSON stringified fallback
- Integration: run a simulated `readContent` tool that returns MCPContent[] and assert saved history message contains readable text.

## Rollback and risk

- The mitigation patch is low-risk: it only affects how tool result content is stringified prior to storage. It does not change the worker/proxy/provider behavior.
- Rollback: revert the single-line change in `ChatContext.tsx` or revert the small helper function.

## Next steps for the engineering team

1. Apply the diagnostic log (first immediate action). Run the same readContent scenario and attach the raw `mcpResponse` (full JSON) to this ticket.
2. If raw response shows MCPContent[], apply the mitigation patch and verify the UI displays readable file content. Add unit tests.
3. If raw response shows a different shape, follow that lead (inspect unified MCP wrapper / provider code) and adjust types/serializers accordingly.

## Contact

If you need additional context or I should apply the diagnostic patch and mitigation patch directly, reply with which option you want (A = diagnostic log only, B = diagnostic + mitigation) and I will open a PR with tests.

## Appendix: Helpful code locations

- `src/lib/web-mcp/mcp-worker.ts` — worker implementation and tool handlers
- `src/lib/web-mcp/mcp-proxy.ts` — proxy that communicates with worker
- `src/features/tools/WebMCPToolProvider.tsx` — provider that calls proxy and returns WebMCP responses
- `src/context/ChatContext.tsx` — ToolCaller and submit/persistence logic (primary suspect)
- `src/hooks/use-ai-service.ts` — stream handling and final message assembly
- `src/lib/ai-service/gemini.ts` — provider streaming & tool-call generation
- `src/lib/message-preprocessor.ts` — attaches file metadata to user messages
