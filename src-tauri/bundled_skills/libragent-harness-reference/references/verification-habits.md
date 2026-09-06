# Verification habits

These are **habits**, not mandatory system-prompt law. Prefer workspace `agents.md` (via **agent-init**) when the user wants durable project rules.

## Prefer evidence over training recall

- For environment facts (files, commands, installed tools, current git state), use tools and report what they returned.
- If you cannot verify, say so explicitly.

## Edit loop

- Read current content before write/edit when the path may already exist.
- After destructive or structural changes, re-check with a tool (list, read, test, or build) before claiming success.

## Attention

- Prefer the smallest tool set that can finish the current step.
- Finish one investigative thread before opening another unless blocked.

## Memory

- Conversation context is limited. For long multi-step work, persist goals/findings with planning/knowledge/scratchpad tools as appropriate — and remember scratchpad is session-local.
