---
title: Sessions
description: "Find, bookmark, and delete sessions; regenerate the last turn; manage workspace overrides and /reload."
---

# Sessions

> A **session** is one conversation. Start, bookmark, delete, and search from the sidebar **Chat** / **History** / **Bookmarked** areas.

---

## Start a new session

1. Open sidebar **Chat**
2. Click a **Built-in Assistants** or **My Assistants** card
3. Send a message in the **New Session** draft

There is no standalone **+ New Session** button flow.

---

## Bookmark

To reopen important sessions quickly:

1. Turn on the bookmark icon on the **History** session card.
2. Open them from sidebar **Bookmarked**, or use the bookmarked filter in History.

---

## Delete

- **No child (sub-agent) sessions**: confirm, then delete that session only.
- **Has children**:
  - **Delete all** (+N subagents) — delete parent and children together
  - **Delete only this** (Subagents kept) — delete parent only; children remain

Deleted sessions cannot be restored. Sub-agents: [Sub-agents](sub-agents.md)

---

## Find & resume

**History** searches name/ID and content on the server (cursor pagination, bookmark/status filters). Open a card to continue.  
Recently switched sessions stay warm in memory so the chat view switches without a full reload.  
During a session, the Chat picker changes **Provider/Model for this session only**.

### Workspace override & `/reload`

- A session **workspace override** (custom work folder) is kept even if the path is briefly unavailable; it applies again when the directory returns.
- Type **`/reload`** in chat to refresh workspace tools and skills without wiping history.

### Rich content in chat

- **Mermaid** (`` ```mermaid ``) and **LaTeX** (`$…$` / `$$…$$`) render in chat messages, deliverable cards, and interactive markdown panels.
- Per-message **Copy** / **Export PDF** keep diagrams and equations (PDF embeds Mermaid as images; math uses the PDF TeX path).

### Regenerate last turn

When the session is **idle** (not streaming), the latest assistant message shows a **Regenerate** action. It discards that completed turn and re-runs from the preceding user message. Use this when you want another take without pasting the same prompt again. Error-path retry remains available when a turn fails.

---

## Export

Use the **Export** control on the chat header or a History card to save the stored conversation:

- **Markdown (.md)** — human-readable transcript (including thinking and tool-use summaries). Recommended for everyday sharing and archiving.
- **ATIF (.json)** — **optional / advanced**. Developer and analysis trajectory format. Large media (screenshots, audio) is stored as placeholders, not raw payloads. See [Session Export](session-export.md) for details.
- **Copy visible window** (active chat only) — copies what you see on screen to the clipboard. This path skips thinking and is size-limited; it is not the same as the Markdown file export.

Busy sessions can still be exported as a point-in-time snapshot. In-flight streaming rows are omitted.

---

## Related

- [First agent chat](../getting-started/first-agent.md)
- [Assistants](assistants.md)
- [Sub-agents](sub-agents.md)
- [Troubleshooting](troubleshooting.md)
