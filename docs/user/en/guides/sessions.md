---
title: Sessions
description: 'Find, bookmark, and delete sessions; regenerate the last turn; manage workspace overrides and /reload.'
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

- **Mermaid** (` ```mermaid `) and **LaTeX** (`$…$` / `$$…$$`) render in chat messages, deliverable cards, and interactive markdown panels.
- Per-message **Copy** / **Export PDF** keep diagrams and equations (PDF embeds Mermaid as images; math uses the PDF TeX path).

### Regenerate last turn

When the session is **idle** (not streaming), the latest assistant message shows a **Regenerate** action. It discards that completed turn and re-runs from the preceding user message. Use this when you want another take without pasting the same prompt again. Error-path retry remains available when a turn fails.

---

## Docker sessions

In the **New Session** draft, turn on **Use Docker Container** to run the session's shell commands and stdio MCP servers in a container instead of on your computer.

- **Default image**: when you set no image (preset **LibrAgent**), the session uses `ghcr.io/fritzprix/libragent-session:<version>`. It contains bash, git, node with npm and npx, python3, and uv with uvx. It does not contain the docker CLI or build tools.
- **Size**: the compressed download is about 152 MB (linux/amd64 and linux/arm64). The first session pulls the image once.
- **Version**: the tag is the LibrAgent version. A development or pre-release build uses the tag of the last released version. The first published image is for LibrAgent 0.9.30; a development build made before that release has no default image. If that image is not available, the session start fails with an error that names the image. Set a different image to continue.
- **Different image**: type an image name or select a preset. The session stores the image, so an app update does not change it. If the image does not have a runtime that a tool needs (for example `npx`), the session reports this when the tool starts.

### Root commands (`asRoot`)

The agent can run a `runShell` command as root with `asRoot`, for example to install a system package (`apt-get install -y nodejs`).

- **Where**: only in a container that LibrAgent creates for the session. A session on your computer or in an attached container refuses `asRoot` and shows no approval card.
- **Approval**: root itself asks for no approval, in every execution mode. A command that needs a hard approval or that is blocked without root keeps that approval or block with root.
- **Protection**: the container runs without the `SETFCAP`, `MKNOD`, and `NET_RAW` capabilities and with `no-new-privileges`. After each root command, LibrAgent stops the processes that the command left and gives the workspace files back to you: it removes setuid/setgid bits and device nodes, and changes root-owned files to your user.
- **Workspace**: the workspace of a container session must not be your home directory or a folder that contains it. Select a project folder.

Accepted residual risk:

- A root command can change the container file system (programs, `/etc`). These changes stay in the session container and go away when LibrAgent creates the container again.
- A command can change the files in the workspace, with or without root, because the container runs as your user.
- With rootful Docker and no user namespace remapping, these protections do not stop a kernel exploit that escapes the container.

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
