---
title: Sessions
---

# Sessions

> Bookmark important chats and delete sessions safely (including ones with sub-agents).

---

## Find & Switch Sessions

- Recent list on **Chat** / sidebar
- Full list: **History**

Recently active sessions are retained warm in memory, making switching between sessions instant without reload latency.

---

## Export

Use the **Export** control on the chat header or a History card to save the stored conversation:

- **Markdown (.md)** — full untruncated transcript from storage, including thinking and tool calls. Compaction/recovery scaffolding is omitted.
- **ATIF trajectory (.json)** — Harbor-aligned ATIF-v1.7 for analysis. Binary media (screenshots, audio) is stored as placeholders, not raw payloads.
- **Copy visible window** (active chat only) — copies the in-memory window to the clipboard. This path skips thinking and is size-limited; it is not the same as the Markdown file export.

Busy sessions can still be exported as a point-in-time snapshot. In-flight streaming rows are omitted.

---

## Bookmark

Use the bookmark control on the session card or header to pin important work.

---

## Delete

Deleting a session is **permanent**.

### Sessions with sub-agents

Some sessions spawn child agents. When you delete a parent:

- The UI may ask whether to delete **with sub-agents** or keep children.
- Prefer the option that matches your intent — orphaned children can clutter History.

Exact wording follows the product (often a confirmation dialog).

---

## Related

- [Sub-agents](sub-agents.md) · [First agent chat](../getting-started/first-agent.md)
