# Tool Use: OpenAI vs. Claude vs. Gemini — Forcing Tool Use (concrete)

This document summarizes how to force or control tool/function use for the three providers we support. It contains the request fields, behavior notes, and short example request snippets.

## OpenAI — tool_choice / function_call

Key fields: `tool_choice` and `function_call` (SDK names vary; platform docs describe `function_call`; newer SDKs expose `tool_choice` modes).

- `tool_choice: "auto"` (default) — model may call zero, one, or multiple functions.
- `tool_choice: "required"` — require the model to call one or more functions.
- `tool_choice: { "type": "function", "name": "get_weather" }` — force the model to call the named function.
- `allowed_tools: ["get_weather","foo"]` — restrict the set of functions the model may call (server-side filter).

Notes:

- Forcing a function by name: the model will still emit the JSON arguments. Your code must execute the function/tool using those arguments and feed results back to the model for follow-up turns.
- `required` requests that the model call at least one function (useful when you want to guarantee tool usage but don't care which one).

Example (minimal request shape):

```json
{
  "model": "gpt-4o",
  "messages": [{ "role": "user", "content": "Get me the weather" }],
  "functions": [
    {
      "name": "get_weather",
      "parameters": {
        /* JSON Schema */
      }
    }
  ],
  "tool_choice": { "type": "function", "name": "get_weather" }
}
```

If you want to require some tool call but not a specific one:

```json
{ "tool_choice": "required" }
```

## Claude / Anthropic — tool_choice (tool use)

Controlling Claude’s output and forcing tool use

You can request Claude to use a specific tool by providing `tool_choice`:

```text
tool_choice = {"type": "tool", "name": "get_weather"}
```

`tool_choice` options:

- `auto` — Claude decides whether to call any provided tools (default when `tools` are present).
- `any` — Claude must use one of the provided tools (but not a specific one).
- `tool` — force Claude to use a specific tool (with `{ type: "tool", name }`).
- `none` — prevent Claude from using any tools (default when no `tools` are present).

Notes from the docs:

- When `tool_choice` is `any` or a specific `tool`, the assistant message will often be prefilled to force a `tool_use` content block. In that case you must return a matching `tool_result` in a following message. The model may skip a natural-language preamble.
- `tool_choice` values `any` and `tool` may be incompatible with Anthropic's "extended thinking" features on some models; prefer `auto` + explicit user instruction if you need both.

Example (minimal request shape):

```json
{
  "model": "claude-3-sonnet",
  "messages": [
    { "role": "user", "content": "Find the current weather for London" }
  ],
  "tools": [
    {
      "name": "get_weather",
      "description": "...",
      "input_schema": {
        /* ... */
      }
    }
  ],
  "tool_choice": { "type": "tool", "name": "get_weather" }
}
```

To let Claude pick some tool (not a specific one):

```json
{ "tool_choice": "any" }
```

To explicitly prevent tool use:

```json
{ "tool_choice": "none" }
```

## Gemini — function_calling_config (tool_config)

Key field: `function_calling_config` inside the call `config`/`tool_config` (names vary by SDK).

- `mode: "AUTO"` (default) — model may return natural language or a function call.
- `mode: "ANY"` — model is constrained to return a function call (guaranteed to follow declared function schema).
- `mode: "NONE"` — disable function calling.
- `allowed_function_names: ["f1","f2"]` — when present with `ANY`, restricts which functions may be chosen.

Example (minimal request shape):

```json
{
  "model": "gemini-1.5",
  "config": {
    "function_calling_config": {
      "mode": "ANY",
      "allowed_function_names": ["get_weather"]
    }
  },
  "contents": [{ "role": "user", "text": "What's the weather in Tokyo?" }],
  "tools": [
    /* function declarations */
  ]
}
```

## Practical comparison & recommendations

- Naming: OpenAI → `tool_choice` / `function_call`; Anthropic/Claude → `tool_choice`; Gemini → `function_calling_config`.
- Specificity: All three let you force a specific function/tool by name. Claude and Gemini provide a way to force "some" tool (Claude: `any`, Gemini: `mode: "ANY"`). OpenAI supports `required` to require one or more functions.
- Natural language: forcing a tool can change conversational behavior (Claude often pre-fills an assistant `tool_use` block and may skip natural-language explanations). If you need both a natural-language explanation and a tool call, prefer:
  - Claude: `tool_choice: "auto"` + explicit instruction in the user message.
  - OpenAI/Gemini: `tool_choice: "auto"`/`function_call: "auto"` + prompt instruction.
- Advanced features: Claude's `tool_choice: any|tool` may be incompatible with "extended thinking" in some models; consult Anthropic docs and test on the target model.

## Implementation notes for our codebase

- Add a high-level option `forceToolUse` on `streamChat` that maps to provider parameters:
  - OpenAI: `tool_choice` ("auto"|"required"|{"type":"function","name":...}) and `allowed_tools` for restrictions.
  - Anthropic/Claude: `tool_choice` ("auto"|"any"|{"type":"tool","name":...}|"none").
  - Gemini: `function_calling_config.mode` ("AUTO"|"ANY"|"NONE") and `allowed_function_names` when forcing a specific name.
- Be conservative: when forcing tool use across providers, preserve system prompts or user instructions that clarify why the tool must be used. For Claude, prefer `auto` + explicit user instruction when you also need human-readable context.
- Validate tool/function names against the converted provider `tools` list before setting the forced-name option; fall back to forcing `any`/`required` (when supported) or `auto` if the name is not found.

## Quick decision table

- Want to force a specific named tool:
  - OpenAI: `tool_choice: {"type":"function","name":"tool_name"}`
  - Claude: `tool_choice: {"type":"tool","name":"tool_name"}`
  - Gemini: `function_calling_config.mode: "ANY"` + `allowed_function_names: ["tool_name"]`
- Want to force "some" tool but not a specific one:
  - OpenAI: `tool_choice: "required"`
  - Claude: `tool_choice: "any"`
  - Gemini: `function_calling_config.mode: "ANY"` (no allowed list)

---

References:

- OpenAI function calling docs: [OpenAI function calling docs](https://platform.openai.com/docs/guides/function-calling)
- Anthropic/Claude tool use docs: [Anthropic/Claude tool use docs](https://docs.claude.com/en/docs/agents-and-tools/tool-use/implement-tool-use)
- Gemini function calling docs (provider SDK docs)
