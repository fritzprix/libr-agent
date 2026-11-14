# TUI parity spec

Status: §1 is the GUI inventory for #2749 (part of epic #2698). §2 (classification and priority) is added by #2750. §3 (command and screen design) and §4 (E2E scenarios) are still to be added by #2751 and #2752.

Goal of the whole spec: `libragent --tui` is functionally near-identical to the GUI. Menus are entered with `/{command}` (for example `/settings`), and sessions are switched with `/new` and `/resume`.

## 1. GUI inventory

This section lists every way a user reaches a feature in the GUI as of `dev/0.9.x` on 2026-11-14. It does not prioritize and does not design the TUI.

Conventions:

- **Entry path** is how a user gets there, for example `Sidebar > History > session card > Export`.
- **Source** paths are relative to the repository root.
- Labels are the English strings from `src/locales/en/common.json`. Where no English key exists, the inline default from the code is used.
- Items marked **dev-only**, **hidden**, or **conditional** are flagged in place and collected in §1.12.

The GUI is one React window (`src/app/App.tsx`). There is no tray icon, native application menu, or second window.

### 1.1 Route and settings coverage

Every route in `src/app/App.tsx` and every Settings tab, with the inventory section that covers it.

| Route / panel             | Component                                              | Covered in         |
| ------------------------- | ------------------------------------------------------ | ------------------ |
| `/`                       | Redirects to `/agent`                                  | —                  |
| `/agent`                  | `src/features/agent/AgentStartRoute.tsx`               | §1.3.1             |
| `/agent/draft`            | `src/features/agent/AgentDraftChatView.tsx`            | §1.3.2             |
| `/agent/:sessionId`       | `src/features/agent/AgentSessionRoute.tsx`             | §1.3.3–§1.3.10     |
| `/assistants`             | `src/features/assistant/AssistantListRoute.tsx`        | §1.5               |
| `/playbooks`              | `src/features/playbook/List.tsx`                       | §1.6               |
| `/history`                | `src/features/history/History.tsx`                     | §1.4.2             |
| `/history/search`         | Same component as `/history`                           | §1.4.2 (hidden)    |
| `/org`                    | `src/features/history/Org.tsx`                         | §1.4.7             |
| `/settings`               | `src/features/settings/SettingsPage.tsx`               | §1.10              |
| `/settings/migration`     | `src/features/migration/MigrationPage.tsx`             | §1.11              |
| `/mcp-servers`            | `src/features/mcp-servers/MCPServerPage.tsx`           | §1.7               |
| `/knowledge`              | `src/features/knowledge/KnowledgePage.tsx`             | §1.8               |
| `/scheduled-tasks`        | `src/features/scheduled-tasks/ScheduledTasksRoute.tsx` | §1.9               |
| Settings > General        | `src/features/settings/tabs/GeneralTab.tsx`            | §1.10.2            |
| Settings > AI & Models    | `src/features/settings/tabs/AIModelsTab.tsx`           | §1.10.3            |
| Settings > Chat Interface | `src/features/settings/tabs/ChatInterfaceTab.tsx`      | §1.10.4            |
| Settings > System         | `src/features/settings/tabs/SystemTab.tsx`             | §1.10.5            |
| Settings > Advanced       | `src/features/settings/tabs/AdvancedTab.tsx`           | §1.10.6            |
| Settings > Experimental   | `src/features/settings/tabs/ExperimentalTab.tsx`       | §1.10.7            |
| Settings > Dev            | `src/features/settings/tabs/DevTab.tsx`                | §1.10.8 (dev-only) |

Feature folders with no route of their own: `src/features/recipes` (one dialog, §1.9), `src/features/skills` (the Extensions > Skills tab, §1.7.2), `src/features/spotlight` (tip cards, §1.2.4), `src/features/app-control` (no UI, §1.2.3), `src/features/mcp` (types only).

### 1.2 App shell

#### 1.2.1 Sidebar

Source: `src/components/layout/AppSidebar.tsx`

| Name             | Entry path                           | What it does                                                                    | Source                          |
| ---------------- | ------------------------------------ | ------------------------------------------------------------------------------- | ------------------------------- |
| History          | Sidebar > History                    | Opens `/history`                                                                | `AppSidebar.tsx`                |
| Bookmarked       | Sidebar > Bookmarked                 | Opens `/history#bookmarked-sessions`; count badge when expanded                 | `AppSidebar.tsx`                |
| Chat             | Sidebar > Chat                       | Opens `/agent`                                                                  | `AppSidebar.tsx`                |
| Knowledge        | Sidebar > Library > Knowledge        | Opens `/knowledge`                                                              | `AppSidebar.tsx`                |
| Assistants       | Sidebar > Library > Assistants       | Opens `/assistants`                                                             | `AppSidebar.tsx`                |
| Playbooks        | Sidebar > Library > Playbooks        | Opens `/playbooks`                                                              | `AppSidebar.tsx`                |
| Extensions       | Sidebar > Library > Extensions       | Opens `/mcp-servers`                                                            | `AppSidebar.tsx`                |
| Org              | Sidebar > Library > Org              | Opens `/org`                                                                    | `AppSidebar.tsx`                |
| Scheduled Tasks  | Sidebar > Library > Scheduled Tasks  | Opens `/scheduled-tasks`                                                        | `AppSidebar.tsx`                |
| Recent Sessions  | Sidebar > Recent Sessions > row      | Opens `/agent/:id`; row shows a status dot and a bookmark icon (§1.4)           | `AppSidebar.tsx`                |
| Settings         | Sidebar footer > Settings            | Opens `/settings`; shows `v{version}` and a red dot when an update is available | `AppSidebar.tsx`                |
| Collapse sidebar | Ctrl/Cmd+B, or header sidebar button | Toggles the sidebar; state kept in the `sidebar_state` cookie                   | `src/components/ui/sidebar.tsx` |

#### 1.2.2 Header

| Name               | Entry path        | What it does                                                                                                                                                                            | Source                                               |
| ------------------ | ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| Sidebar toggle     | Header, left      | Collapses or expands the sidebar                                                                                                                                                        | `src/components/ui/sidebar.tsx` (`SidebarTrigger`)   |
| Notifications bell | Header > bell     | "Notifications" dropdown with unread badge. Row opens the session and marks it viewed; badges "Recurring stop" and "{{count}} approval(s)"; X dismisses one; **Mark all read** (§1.4.6) | `src/components/layout/SessionNotificationsBell.tsx` |
| Theme toggle       | Header > sun/moon | Switches light and dark                                                                                                                                                                 | `src/components/common/ThemeToggle.tsx`              |

#### 1.2.3 Global overlays, events, and shortcuts

| Name                         | Entry path                                                        | What it does                                                                                                                                     | Source                                                       |
| ---------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ |
| Agent Pairing Request dialog | Opens by itself on a `session_api_pairing_requested` event        | Approve or reject Session API access for an external agent: countdown, "Pairing Code (xxx-yyy)" field, **Approve** / **Reject**; closing rejects | `src/features/agent/components/SessionApiPairingDialog.tsx`  |
| Update toast                 | Automatic, about 5 s after launch (**dev-only skip**: not in dev) | "LibrAgent X is available" with **Install** (download, install, restart) or **View changelog** / **Later**                                       | `src/context/UpdateContext.tsx`                              |
| Toasts                       | Top right                                                         | Global notification toasts                                                                                                                       | `src/app/App.tsx` (`Toaster`)                                |
| UI scale shortcuts           | Ctrl/Cmd + `+`/`=`, `-`, `0`                                      | Steps zoom through 90/100/110/125/150 %; `0` resets; saved to settings                                                                           | `src/hooks/use-ui-scale-shortcuts.ts`, `src/lib/ui-scale.ts` |
| Agent panel shortcuts        | Ctrl/Cmd+Shift+J / P / U on agent screens                         | Opens Processes / Planning / Workspace panel (§1.3.10)                                                                                           | `src/features/agent/hooks/usePanelShortcuts.ts`              |
| App control bridge           | **Hidden**: backend event `libragent:app-control` only            | Remote navigate, focus a session, highlight or install an Extensions preset                                                                      | `src/features/app-control/useAppControlBridge.tsx`           |

There is no global command palette. `src/features/spotlight` holds only tip cards.

#### 1.2.4 Feature tips (spotlight)

Shown only when Settings > General > **Show feature tips** is on (`display.showFeatureTips`). Dismissals are kept in localStorage (`libragent:spotlight-state:v1`).

| Name                         | Entry path                                    | What it does                                           | Source                                                       |
| ---------------------------- | --------------------------------------------- | ------------------------------------------------------ | ------------------------------------------------------------ |
| "What's new in v{{version}}" | Chat start screen, first run of a new version | Up to 3 new-feature tips with CTA buttons; **Dismiss** | `src/features/spotlight/components/ReleaseSpotlightCard.tsx` |
| "Tip" card                   | Chat start screen, when no release card       | One rotating tip: CTA, **Next tip**, X to hide         | `src/features/spotlight/components/HubHintCard.tsx`          |
| "Tip · {{title}}" link       | Chat composer while the agent is busy         | Runs the tip's CTA                                     | `src/features/spotlight/components/WaitTipLink.tsx`          |

Tip CTAs and targets are defined in `src/features/spotlight/feature-spotlights.ts`. They open `/agent`, `/org`, `/knowledge`, `/scheduled-tasks`, or a Settings tab (`?tab=general`, `?tab=system`, `?tab=ai-models`). One CTA ("Create nightly distill task") creates a scheduled task, then opens `/scheduled-tasks`.

### 1.3 Chat

All sources in this section are under `src/features/agent/` unless they start with `src/`.

#### 1.3.1 Start screen (`/agent`)

| Name                         | Entry path                                                    | What it does                                                                                | Source                                             |
| ---------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| AI model banner              | Start (conditional: no provider configured)                   | "AI Model Configuration Required"; **Configure AI Model** opens `/settings?tab=ai-models`   | `AgentChatStartView.tsx`                           |
| Runtime banner               | Start (conditional: npx not ready)                            | **Open App Wizard**, **Recheck**, **Restart app**; dismissal kept in localStorage           | `AgentChatStartView.tsx`                           |
| Feature tips                 | Start                                                         | See §1.2.4                                                                                  | `src/features/spotlight/*`                         |
| Morning Briefing recipe card | Start > "Create Your Morning Briefing Assistant in 5 Minutes" | **Start Guided Walkthrough** opens the walkthrough dialog (§1.9.3); X dismisses             | `AgentChatStartView.tsx`, `src/features/recipes/*` |
| Built-in Assistants          | Start > Built-in Assistants > card                            | Opens `/agent/draft?assistantId=…`                                                          | `components/AssistantSelectionCard.tsx`            |
| My Assistants                | Start > My Assistants > card                                  | Opens `/agent/draft?assistantId=…`                                                          | `components/AssistantSelectionCard.tsx`            |
| Manage Assistants            | Start > "+ Manage Assistants"                                 | Opens `/assistants`                                                                         | `AgentChatStartView.tsx`                           |
| Empty / error state          | Start (conditional)                                           | "No assistants available yet." with **Create Assistant**; "Failed to load assistants."      | `AgentChatStartView.tsx`                           |
| Playbook start               | `/agent?playbookId=…` (from Playbooks > Start)                | Opens the playbook's pinned session, or creates a new session named after the playbook goal | `AgentChatStartView.tsx`                           |

#### 1.3.2 Draft screen (`/agent/draft`)

A draft is a chat before a session exists. The first send creates the session.

| Name                | Entry path                                                       | What it does                                                                                                                                                             | Source                                                                                                  |
| ------------------- | ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- |
| Draft header        | Draft > header                                                   | "[assistant] · New Session"; no rename, bookmark, or export                                                                                                              | `components/AgentSessionHeader.tsx`                                                                     |
| Capabilities        | Draft > capabilities                                             | "Basic Tools" badge, badges for optional built-in services and MCP servers                                                                                               | `components/DraftCapabilitiesSection.tsx`                                                               |
| Add tools           | Draft > "+ Add tools"                                            | Opens the assistant editor on the Tools tab (§1.5); changes are saved to the assistant                                                                                   | `components/DraftCapabilitiesSection.tsx`                                                               |
| Model and thinking  | Draft > capabilities > model picker                              | Provider and model for the new session; thinking effort Off / Low / Medium / High / Auto (writes the global `advanced.thinkingEffort`)                                   | `components/DraftCapabilitiesSection.tsx`, `src/features/settings/components/ThinkingEffortControl.tsx` |
| Isolation Settings  | Draft > Advanced Configuration > Isolation Settings              | **Use Docker Container** switch, **Docker Image** input with preset chips, Docker missing / daemon offline warnings. Hidden while a workspace override is set            | `components/WorkspaceIsolationSettings.tsx`                                                             |
| Workspace by drop   | Draft > drop a folder ("Drop a folder here to set as workspace") | Sets the workspace for the new session and opens a read-only preview panel (refresh, close to clear, isolation settings)                                                 | `hooks/useAgentDraftChat.ts`, `components/AgentDraftWorkspacePreviewPanel.tsx`                          |
| Draft composer      | Draft > input                                                    | Attach (paperclip), paste image, drop files; Enter sends, Shift+Enter newline. `@` offers only skills and files                                                          | `AgentDraftChatView.tsx`                                                                                |
| Create on send      | Draft > Send                                                     | Creates the session (name = first 50 characters), attaches files, sends, opens `/agent/:id`                                                                              | `hooks/useAgentDraftChat.ts`                                                                            |
| No-provider toast   | Draft > Send (conditional)                                       | Error toast with a Settings action                                                                                                                                       | `hooks/useAgentDraftChat.ts`                                                                            |
| Docker error dialog | Draft > Send (conditional: Docker fails)                         | "Docker Engine Offline" / "Docker is not installed": **Start Docker Desktop**, **Run in Host Mode**, **Retry Session Start**, **Cancel**, "Download Docker Desktop" link | `components/DockerErrorModal.tsx`                                                                       |

#### 1.3.3 Session screen (`/agent/:sessionId`)

| Name                 | Entry path                                  | What it does                                                                                                            | Source                                                            |
| -------------------- | ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Loading overlay      | Session > while loading                     | "Loading session…", "Starting session…", "Preparing Docker workspace ({{image}})"; per-MCP-server status with **Retry** | `components/SessionLoadingOverlay.tsx`                            |
| Load error           | Session (conditional)                       | "Session not found or failed to load."                                                                                  | `AgentChatView.tsx`                                               |
| Playbook auto-start  | `/agent/:sessionId?playbookId=…`            | Selects the playbook and sends the start instruction; "Starting playbook…" strip                                        | `AgentChatView.tsx`                                               |
| MCP discovery toasts | Session startup (conditional: external MCP) | "Loading tools...", then ready / partly failed / failed                                                                 | `hooks/useMcpDiscoveryToasts.ts`                                  |
| Keep-alive           | Switching sessions                          | The 3 most recent sessions stay mounted in the background; a deleted active session redirects to `/agent`               | `AgentSessionKeepAliveHost.tsx`, `hooks/useRetainedSessionIds.ts` |

#### 1.3.4 Session header

| Name                      | Entry path                                                             | What it does                                                                                                        | Source                                                                |
| ------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Title                     | Session > header                                                       | Assistant name and session title                                                                                    | `components/AgentChatHeader.tsx`, `components/AgentSessionHeader.tsx` |
| Rename session            | Header > pencil                                                        | Inline edit; Enter saves, Esc cancels; empty title is refused                                                       | `components/AgentSessionHeader.tsx`                                   |
| Bookmark                  | Header > bookmark icon                                                 | Toggles the bookmark                                                                                                | `components/AgentSessionHeader.tsx`                                   |
| Export session            | Header > download icon                                                 | "Export as Markdown (.md)", "Export as ATIF trajectory (.json)", "Copy visible window (no thinking / size-limited)" | `components/SessionExportMenu.tsx`                                    |
| Copy pairing instructions | Header > key icon ("Integrate into Agent Session (copy instructions)") | Copies instructions for an external agent to pair with this session                                                 | `components/AgentChatHeader.tsx`, `lib/pairingInstruction.ts`         |
| Panel update badges       | Header > "{{panel}} has updates — click to open"                       | Opens the panel that changed                                                                                        | `components/AgentChatHeader.tsx` (`HeaderStatusBadges`)               |
| Toggle agent panels       | Header > panel icon                                                    | Opens or closes the side panels (§1.3.10)                                                                           | `components/AgentChatHeader.tsx`                                      |
| Session files             | Header > paperclip with count                                          | "Session file list" dropdown; clicking a file opens a viewer dialog with content and metadata                       | `src/components/shared/SessionFilesPopover.tsx`                       |

#### 1.3.5 Status bar

Source folder: `components/agent-chat-status-bar/`

| Name                     | Entry path                                                    | What it does                                                                                                         | Source                                                                                        |
| ------------------------ | ------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Workflow banner          | Status bar                                                    | State text (ready, processing, awaiting approval, compressing context, paused, error) with **Retry** or **Continue** | `StatusBarWorkflowBanner`                                                                     |
| Session model            | Status bar > Provider / Model                                 | Changes the session model; allowed only when idle, paused, or errored. **Refresh models** reloads the provider list  | `StatusBarModelControls.tsx`, `useStatusBarModelConfig.ts`, `components/AgentModelPicker.tsx` |
| Sub-session batch update | Status bar > model change (conditional: session has children) | "Batch Update Sub-sessions": **Only this session** / **Update sub-sessions too** / **Cancel**                        | `components/BatchModelUpdateDialog.tsx`                                                       |
| Thinking effort          | Status bar > Off / Low / Medium / High / Auto                 | Sets the global reasoning effort                                                                                     | `src/features/settings/components/ThinkingEffortControl.tsx`                                  |
| Execution mode           | Status bar > Execution > Normal / YOLO / Full Auto            | Sets the approval policy for this session (normal, yolo, unsafe)                                                     | `StatusBarExecutionMode.tsx`                                                                  |
| Token metrics            | Status bar > metrics badge                                    | Prompt / completion tokens, cache hit %, tok/s, TTFT; affected by the Performance Metrics settings (§1.10.2)         | `components/TokenMetricsBadge.tsx`                                                            |
| Tools count              | Status bar > "Tools: …"                                       | Opens "Available Tools": each tool with builtin/mcp badge, description, input schema                                 | `StatusBarToolsControl`, `components/AgentToolsModal.tsx`                                     |
| Retry failed MCP         | Status bar > "Retry (n)"                                      | Restarts failed MCP servers for this session                                                                         | `StatusBarToolsControl`                                                                       |

#### 1.3.6 Composer

| Name               | Entry path                                       | What it does                                                                                     | Source                                                    |
| ------------------ | ------------------------------------------------ | ------------------------------------------------------------------------------------------------ | --------------------------------------------------------- |
| Mode badge         | Above input                                      | "YOLO Mode Active" / "Full Auto Active"                                                          | `components/AgentChatInput.tsx`                           |
| Attached files bar | Above input                                      | Pending attachments, each removable                                                              | `components/AgentChatAttachedFiles*`                      |
| Queued prompts     | Above input > "Queued prompts"                   | Messages sent while busy; cancel each; "+{{count}} more"                                         | `LayeredPendingQueue`                                     |
| Input              | Input                                            | Enter sends (queues while busy), Shift+Enter newline, Enter ignored during IME composition       | `components/AgentChatInput.tsx`, `hooks/useChatSubmit.ts` |
| `@` mentions       | Input > type `@`                                 | `@skill:` (with source badge), `@file:`, `@tool:` (top 10), `@playbook:`; arrows, Enter/Tab, Esc | `hooks/useInputToken.ts`, `InputTokenDropdown`            |
| `/clear`           | Input > type `/clear`                            | Resets the session                                                                               | `hooks/useInputToken.ts`, `hooks/useChatSubmit.ts`        |
| `/reload`          | Input > type `/reload`                           | Backend reload command                                                                           | `hooks/useInputToken.ts`                                  |
| `/permission …`    | Input > `/permission yolo` / `unsafe` / `normal` | Sets the execution mode                                                                          | `hooks/useInputToken.ts`                                  |
| Attach file        | Input > paperclip                                | File picker limited to supported types                                                           | `hooks/useAgentFileAttachment.ts`                         |
| Drop files         | Input > drop                                     | Attaches files; placeholder shows supported / unsupported                                        | `components/AgentChatInput.tsx`                           |
| Paste image        | Input > paste                                    | Attaches the clipboard image                                                                     | `useClipboardImage`                                       |
| Cancel request     | Input > stop button, or Esc                      | Cancels the workflow and pauses. Esc is skipped while an approval, prompt, or dialog is open     | `components/AgentChatInput.tsx`                           |
| Resume             | Input > play button (conditional: paused)        | Resumes the workflow                                                                             | `components/AgentChatInput.tsx`                           |
| Oversize guard     | Input > Send (conditional)                       | Refuses a first message larger than 2 × Max Input Context                                        | `hooks/useChatSubmit.ts`                                  |

Slash commands and `@` types have no visible button; the user finds them only by typing.

#### 1.3.7 Messages

| Name              | Entry path                                              | What it does                                                                                                                                | Source                                               |
| ----------------- | ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| Load older        | Top of list                                             | Loads earlier messages (50 at a time)                                                                                                       | `components/AgentChatMessages.tsx`                   |
| Jump to approvals | Floating button (conditional)                           | "{{count}} tools awaiting approval — jump to respond"                                                                                       | `components/AgentChatMessages.tsx`                   |
| Scroll to latest  | Floating button                                         | Scrolls to the bottom                                                                                                                       | `components/AgentChatMessages.tsx`                   |
| Compact divider   | List > "Context compressed above"                       | Expandable summary of condensed messages                                                                                                    | `components/shared/CompactEventDivider.tsx`          |
| Role label        | Message header                                          | "You", assistant name, or "Notification" with "CHANNEL" badge                                                                               | `components/AgentMessageBubble.tsx`                  |
| Copy message      | Message > action bar > Copy                             | Copies the text. Bar shows on hover (Document layout) or always (Bubble layout)                                                             | `components/MessageActionBar.tsx`                    |
| Regenerate        | Message > action bar (latest assistant turn, idle only) | Deletes the last turn and runs it again                                                                                                     | `components/MessageActionBar.tsx`                    |
| Copy tool calls   | Message > action bar                                    | Copies the message's tool calls as JSON                                                                                                     | `components/MessageActionBar.tsx`                    |
| Export message    | Message > action bar > Export                           | Markdown (or HTML for HTML documents) and PDF                                                                                               | `components/MessageActionBar.tsx`                    |
| Error bubble      | List footer (conditional)                               | **Try Again**, **Configure API Key in Settings**, or **Open Context Settings** depending on the error                                       | `src/components/shared/ErrorBubble.tsx`              |
| Thinking          | Message > "Thinking Process"                            | Collapsed preview; expand; auto-expands while streaming                                                                                     | `components/shared/ThinkingBubble.tsx`               |
| Rich content      | Message body                                            | Markdown, highlighted code, Mermaid diagrams (with show source on error), images (Copy / Download), audio and video players, resource links | `AgentMessageRenderer/*`, `components/CodeBlock.tsx` |

There is no edit, fork, branch, or delete action on messages.

#### 1.3.8 Tool calls and result cards

| Name                | Entry path                                 | What it does                                                                                                                         | Source                                                                          |
| ------------------- | ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------- |
| Tool group          | List > "Tool Executions ({{count}} calls)" | Running/success/error badges; **Show All** / **Show Less** for older calls                                                           | `components/AgentToolCallGroup.tsx`                                             |
| Tool call row       | Tool group > row                           | Status, name, expand to "Parameters" and "Result" / "Error Details". Expand rules depend on **Tool Detail Level** (§1.10.2)          | `components/ToolCallCompactItem.tsx`, `components/AgentToolCallDetails.tsx`     |
| MCP UI resource     | Tool result > embedded UI                  | Renders MCP UI; actions are tool call, intent, prompt, link, notify; interactive answers continue the agent                          | `AgentMessageRenderer/hooks/useUIActionHandler.ts`                              |
| Legacy form         | Tool result > `ui://interactive/…`         | Rendered read-only (all controls disabled)                                                                                           | `AgentMessageRenderer/utils/disableLegacyInteractiveForm.ts`                    |
| File write card     | `workspace__writeFile` result              | Created / Overwritten / Appended badge, diff, **Preview**, **Open file**                                                             | `components/tool-structured/FileWriteActions.tsx`                               |
| String replace card | `workspace__strReplace` result             | Occurrence count and diff                                                                                                            | `components/tool-structured/StrReplaceDiffView.tsx`                             |
| Shell output card   | `workspace__runShell` result               | Exit code, duration, working dir, command, stdout, stderr                                                                            | `components/tool-structured/TerminalOutputBlock.tsx`                            |
| Report result card  | `ui__reportResult` result                  | Final / Partial / Blocked, acceptance criteria, verification proof, outcome; deliverables with **Preview** / **Open** / **Download** | `components/tool-structured/ReportResultCard.tsx`, `DeliverableFileActions.tsx` |
| Sub-agent card      | `agent__*Session` tool results             | Child session state, turns, task, result, recent activity; **Open session**, **Copy result**                                         | `components/tool-structured/AgentSessionToolCard.tsx`                           |

#### 1.3.9 In-chat prompts

| Name            | Entry path                                                   | What it does                                                                                                                                                  | Source                                                               |
| --------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| Tool approval   | List footer > "Approval Required" / "Hard Approval Required" | Tool name and JSON arguments. **Reject** (Esc), **Approve** (Ctrl/Cmd+Enter; bare Enter never approves). Hard approvals are targeted first. No "always allow" | `components/PendingApprovalWidget.tsx`                               |
| Secret prompt   | Dialog "A password is needed"                                | Password input for a named variable; **Cancel** / Esc; outside click blocked                                                                                  | `components/UserPromptDialog.tsx`                                    |
| Question prompt | Dialog "Your answer is needed"                               | Single select, multi select, "Other" with text, or free text per question; **Submit** when all answered; **Cancel**                                           | `components/UserPromptDialog.tsx`, `components/AskUserQuestions.tsx` |
| Pairing request | Any screen                                                   | See §1.2.3                                                                                                                                                    | `components/SessionApiPairingDialog.tsx`                             |

#### 1.3.10 Side panels

Right rail with tabs **Workspace**, **Processes**, **Planning**. Source shell: `components/AgentSidePanelShell.tsx`, resize: `components/PanelResizeHandle.tsx` (drag, arrow keys, double-click or Home resets).

| Name               | Entry path                                                            | What it does                                                                                                                   | Source                                                                                 |
| ------------------ | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------- |
| Workspace toolbar  | Workspace > **Open in Explorer** / **Open in Terminal** / **Refresh** | Opens the folder in the OS file manager or a terminal; reloads the tree                                                        | `components/AgentWorkspacePanel.tsx`                                                   |
| Workspace override | Workspace > **Browse...** > **Set** / **Cancel**                      | Binds or unbinds a custom folder for this session; also by dropping one folder on the panel                                    | `components/AgentWorkspacePanel.tsx`, `workspace-panel/useWorkspaceOverride.ts`        |
| File tree          | Workspace > tree                                                      | Expand folders; click a file to preview (or open with default app); drop files on a folder to import; 🐳 DOCKER badge          | `components/AgentWorkspacePanel.tsx`, `workspace-panel/FileTreeNode.tsx`               |
| Upload zone        | Workspace > "Drop files here or click to upload"                      | Imports files into the workspace                                                                                               | `components/AgentWorkspacePanel.tsx`                                                   |
| File preview       | Workspace or tool card > Preview                                      | Copy path, HTML preview/source, **Copy Content**, **Maximize**, **Open in Default App**; image, Markdown, code, binary viewers | `workspace-panel/WorkspaceFilePreviewSheet.tsx`, `components/AgentFilePreviewHost.tsx` |
| Processes          | Processes > list                                                      | **Refresh**, counts; per process **View output**, **Check status**, **Stop**                                                   | `components/AgentProcessPanel.tsx`, `process-panel/ProcessListItem.tsx`                |
| Process output     | Processes > View output                                               | Dialog with status, stream, stdout, stderr                                                                                     | `process-panel/ProcessOutputDialog.tsx`                                                |
| Planning           | Planning > content                                                    | Progress, "Current Goal", "Tasks" with priority badges, "Scratchpad"                                                           | `components/AgentPlanningPanel.tsx`                                                    |
| Session schedules  | Planning > Schedules                                                  | Per schedule: pause/resume switch, cancel X, countdown; no edit                                                                | `SessionSchedulesSection`, `hooks/useSessionSchedules.ts`                              |
| Panel attention    | Background                                                            | Attention dots and an "AI Planning" toast when a hidden panel changes                                                          | `components/AgentProcessAttentionUpdates.tsx`, `components/AgentPlanningUpdates.tsx`   |

### 1.4 Session management

#### 1.4.1 Create

| Name                | Entry path                                   | What it does                                                         | Source                                          |
| ------------------- | -------------------------------------------- | -------------------------------------------------------------------- | ----------------------------------------------- |
| New session         | Sidebar > Chat > assistant card > first Send | Draft flow (§1.3.1, §1.3.2)                                          | `src/features/agent/hooks/useAgentDraftChat.ts` |
| From playbook       | Playbooks > Start                            | §1.3.1 Playbook start                                                | `src/features/agent/AgentChatStartView.tsx`     |
| From scheduled task | Scheduled run (no user action)               | The scheduler reuses the task's pinned session, or creates it (§1.9) | `src-tauri/src/scheduled/runner.rs`             |

Child sessions are created only by the agent's own tools, never by a user action. The assistant of a session is fixed at creation; there is no UI to change it.

#### 1.4.2 List, filter, and search

| Name              | Entry path                                          | What it does                                                                            | Source                                                                                              |
| ----------------- | --------------------------------------------------- | --------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Recent Sessions   | Sidebar > Recent Sessions                           | Paged tree (20 per page), infinite scroll and **Load more**; chevron loads children     | `src/components/layout/AppSidebar.tsx`, `src/components/layout/sidebar-recent-sessions.ts`          |
| History page      | Sidebar > History                                   | "Session History" with refresh, bookmarked strip, controls, session tree, **Load more** | `src/features/history/History.tsx`, `src/features/agent/components/SessionHistoryPanel.tsx`         |
| Search            | History > "Search sessions by name or ID..."        | Server-side search after 300 ms; X clears                                               | `src/features/history/useSessionHistorySearch.ts`                                                   |
| Sort              | History > sort select + direction                   | Recent activity / Created / Name, ascending or descending                               | `session-history-panel/SessionHistoryControls.tsx`                                                  |
| Status filter     | History > Status                                    | All / Busy / Idle / Paused / Error / Queued / Setting up, with counts                   | `session-history-panel/SessionHistoryControls.tsx`                                                  |
| Bookmarked filter | Sidebar > Bookmarked, or History > "Bookmarked (n)" | Shows only bookmarked sessions                                                          | `session-history-panel/SessionHistoryControls.tsx`                                                  |
| Bookmarked strip  | History > Bookmarked section                        | Preview rows with resume and unbookmark; **Browse all bookmarked sessions**             | `session-history-panel/BookmarkedSessionsSection.tsx`                                               |
| Lineage filter    | History > card > "Lineage: <id>"                    | Narrows to one lineage tree; focus bar **Show all lineages**                            | `src/features/agent/components/SessionCard.tsx`, `session-history-panel/SessionHistoryFocusBar.tsx` |
| Tree badges       | History > card                                      | Root/Child, depth, parent, descendants, hidden by filter, not loaded, per-status counts | `src/features/agent/components/SessionCard.tsx`                                                     |
| `/history/search` | **Hidden**: direct URL only; no link in the code    | Same screen as `/history`                                                               | `src/app/App.tsx`                                                                                   |

Paths in `session-history-panel/` are under `src/features/agent/components/`. There is no message-content search in the GUI (`searchMessages` in `src/lib/backend/messages.ts` is not called by any screen).

#### 1.4.3 Resume

| Name                | Entry path                                            | What it does                                | Source                                                                   |
| ------------------- | ----------------------------------------------------- | ------------------------------------------- | ------------------------------------------------------------------------ |
| Open from sidebar   | Sidebar > Recent Sessions > row                       | Opens `/agent/:id`                          | `src/components/layout/AppSidebar.tsx`                                   |
| Resume from History | History > card > **Continue** / **Resume** / **View** | Opens `/agent/:id`; label depends on status | `src/features/agent/components/SessionCard.tsx`                          |
| From notification   | Header > bell > row                                   | Opens the session and marks it viewed       | `src/components/layout/SessionNotificationsBell.tsx`                     |
| From Org            | Org > card > **Resume Root Session**                  | Opens the org root session                  | `src/features/history/OrgCard.tsx`                                       |
| From sub-agent card | Chat > sub-agent card > **Open session**              | Opens the child session                     | `src/features/agent/components/tool-structured/AgentSessionToolCard.tsx` |

#### 1.4.4 Rename, bookmark, delete

| Name                 | Entry path                                                     | What it does                                                                         | Source                                                      |
| -------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ----------------------------------------------------------- |
| Rename               | Session header > pencil only                                   | §1.3.4; no rename in the sidebar or History                                          | `src/features/agent/components/AgentSessionHeader.tsx`      |
| Bookmark             | Session header, History card, Bookmarked strip                 | Toggles the bookmark; the sidebar shows an indicator only                            | `src/context/agent-session-list/useSessionListMutations.ts` |
| Delete               | History > card > trash > **Confirm Delete**                    | Deletes the session                                                                  | `src/features/agent/components/SessionCard.tsx`             |
| Delete with children | History > card > trash > **Delete all** / **Delete only this** | Deletes the session and its subagents, or only this one (children move to top level) | `src/features/agent/components/SessionCard.tsx`             |
| Clear all sessions   | Settings > Advanced > Danger Zone                              | §1.10.6                                                                              | `src/features/settings/components/DangerZoneSettings.tsx`   |

There is no delete from the sidebar, and no archive or pin. Bookmark is the closest to pin.

#### 1.4.5 Export

| Name         | Entry path                                 | What it does                           | Source                                                                                     |
| ------------ | ------------------------------------------ | -------------------------------------- | ------------------------------------------------------------------------------------------ |
| Markdown     | History card or session header > Export    | Saves `.md` (generated by the backend) | `src/features/agent/components/SessionExportMenu.tsx`, `src/lib/backend/session-export.ts` |
| ATIF         | History card or session header > Export    | Saves an ATIF trajectory `.json`       | same                                                                                       |
| Copy visible | Session header > Export only               | Copies loaded messages as Markdown     | `src/features/agent/components/SessionExportMenu.tsx`                                      |
| Org bundle   | Org > card > Export Org / Org > Import Org | §1.4.7                                 | `src/features/history/Org.tsx`                                                             |

#### 1.4.6 Status and notifications

| Name                 | Entry path                       | What it does                                                                                                  | Source                                                      |
| -------------------- | -------------------------------- | ------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| Sidebar status dot   | Sidebar row                      | busy (pulse), queued (pulse), error, idle/paused (dim)                                                        | `src/components/layout/AppSidebar.tsx`                      |
| History status badge | History card                     | Active, Queued, Setting up Docker, Idle, Paused, Error (with reason)                                          | `src/features/agent/components/SessionCard.tsx`             |
| Live updates         | Background                       | Status, last message time, and pending approval counts follow backend `agent:event` events                    | `src/context/agent-session-list/useSessionListEventSync.ts` |
| Attention list       | Header > bell                    | Sessions that need attention (pending approvals, recurring stop); **Mark all read**, dismiss one, empty state | `src/components/layout/SessionNotificationsBell.tsx`        |
| Mark viewed          | Automatic on open, send, approve | Clears the attention state                                                                                    | `src/context/agent-session-list/useSessionListMutations.ts` |

#### 1.4.7 Org view (`/org`)

| Name             | Entry path                       | What it does                                                                                          | Source                                                       |
| ---------------- | -------------------------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| Import Org       | Org > Import Org                 | Picks a `.libragent-org` file, inspects it, imports (merge). Encrypted bundles are refused in this UI | `src/features/history/Org.tsx`, `useOrgBundle.ts`            |
| Refresh / search | Org > header                     | Reloads; filters org cards by name or ID                                                              | `src/features/history/Org.tsx`                               |
| Org card         | Org > card                       | Root status, member and active counts, lineage snapshot (up to 8 nodes)                               | `src/features/history/OrgCard.tsx`, `OrgLineageSnapshot.tsx` |
| Export Org       | Org > card > Export Org          | Folder picker, writes `libragent-org-<name>-<ts>.libragent-org`                                       | `src/features/history/OrgCard.tsx`                           |
| Delete org       | Org > card > trash > Delete      | Deletes the root session and all members                                                              | `src/features/history/OrgCard.tsx`                           |
| Resume root      | Org > card > Resume Root Session | §1.4.3                                                                                                | `src/features/history/OrgCard.tsx`                           |

#### 1.4.8 Workspace binding

| Name                    | Entry path                                       | What it does                                   | Source                                                                  |
| ----------------------- | ------------------------------------------------ | ---------------------------------------------- | ----------------------------------------------------------------------- |
| At creation             | Draft > drop folder; Isolation Settings          | Workspace path, host or Docker, image (§1.3.2) | `src/features/agent/hooks/useAgentDraftChat.ts`                         |
| Override after creation | Session > Workspace panel > Browse / drop folder | §1.3.10                                        | `src/features/agent/components/workspace-panel/useWorkspaceOverride.ts` |
| Scheduled task          | Scheduled Tasks > task > Workspace               | §1.9                                           | `src/features/scheduled-tasks/components/ScheduledTaskModal.tsx`        |

#### 1.4.9 Backend commands used by session screens

For reuse by the TUI. Wrappers are in `src/lib/backend/`, except `agent_execute_command`, which `src/features/agent/hooks/useChatSubmit.ts` invokes directly for slash commands.

| Group       | Tauri commands                                                                                                                                                                                                                                                          |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Lifecycle   | `agent_create_session`, `agent_open_session`, `agent_get_session`, `agent_list_sessions` (`limit`, `cursor`, `search`, `status`, `bookmarkedOnly`), `agent_list_attention_sessions`, `agent_get_all_sessions`                                                           |
| Tree        | `agent_get_child_sessions`, `agent_get_child_session_ids`, `agent_get_descendant_session_ids`                                                                                                                                                                           |
| Change      | `agent_update_session_name`, `agent_toggle_session_bookmark`, `agent_mark_session_viewed`, `agent_update_session_config`, `agent_set_execution_mode`                                                                                                                    |
| Delete      | `agent_delete_session`, `agent_delete_session_only`, `agent_clear_all_sessions`, `agent_factory_reset`                                                                                                                                                                  |
| Run control | `agent_send_message`, `agent_add_attachment`, `agent_cancel_workflow`, `agent_terminate_workflow`, `agent_resume_workflow`, `agent_regenerate_last_turn`, `agent_get_pending_queue`, `agent_cancel_pending_prompt`, `agent_execute_command`, `retry_session_mcp_server` |
| Export      | `export_session_file` (`markdown`, `atif`), `export_org_bundle`, `inspect_org_bundle`, `import_org_bundle`                                                                                                                                                              |
| Workspace   | `get_workspace_override`, `set_workspace_override`, `cancel_workspace_override`, `get_workspace_dir`, `list_workspace_files`, `read_workspace_file_content`, `open_workspace_in_explorer`, `open_workspace_in_terminal`                                                 |
| Schedules   | `list_session_scheduled_tasks`, `toggle_session_scheduled_task`, `cancel_session_scheduled_task`                                                                                                                                                                        |
| Events      | `agent:event` (`statusChanged`, `messageAdded`, `workflowCompleted`, `toolExecutionRequiresApproval`, `toolExecutionApprovalResolved`, `sessionRuntimeStateUpdated`)                                                                                                    |

### 1.5 Assistants (`/assistants`)

| Name             | Entry path                             | What it does                                                                                                                         | Source                                                                 |
| ---------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------- |
| Search           | Assistants > "Search assistants..."    | Filters with result count; X clears                                                                                                  | `src/features/assistant/List.tsx`                                      |
| Create           | Assistants > **Create New Assistant**  | Opens the editor                                                                                                                     | `src/features/assistant/List.tsx`                                      |
| Card             | Assistants > card                      | Expand to see system prompt, description, dates; badges PROTECTED, MCP servers, Built-in Automation, optional tools                  | `src/features/assistant/Card.tsx`                                      |
| Edit             | Card > **Edit**                        | Opens the editor                                                                                                                     | `src/features/assistant/Card.tsx`                                      |
| Delete           | Card > **Delete** > **Confirm Delete** | Deletes; protected assistants are refused                                                                                            | `src/features/assistant/Card.tsx`                                      |
| Editor > General | Editor > General                       | Name, Description, System Prompt                                                                                                     | `src/features/assistant/AssistantEditor.tsx`                           |
| Editor > Tools   | Editor > Tools                         | Switch per optional built-in service; checkbox per active MCP server (with search)                                                   | `src/features/assistant/BuiltInToolsEditor.tsx`, `AssistantEditor.tsx` |
| Editor > Skills  | Editor > Skills (after first save)     | Enable/disable per skill; drop `.skill`/`.zip`/folder to add assistant skills; override or revert a global skill; **Reset Override** | `src/features/assistant/SkillsEditor.tsx`                              |

There is no duplicate action and no context menu.

### 1.6 Playbooks (`/playbooks`)

Playbooks are created by the agent in chat; there is no create button.

| Name           | Entry path                        | What it does                                                                                       | Source                                              |
| -------------- | --------------------------------- | -------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| Refresh        | Playbooks > refresh icon          | Reloads the list                                                                                   | `src/features/playbook/List.tsx`                    |
| Search         | Playbooks > "Search playbooks..." | Filters                                                                                            | `src/features/playbook/List.tsx`                    |
| Display        | Playbooks > Display               | Sort by Date Created / Assistant; order; grouping None / By Time / By Assistant; Bookmarks First   | `src/features/playbook/SortControls.tsx`            |
| Group header   | Playbooks > group                 | Collapse or expand a group                                                                         | `src/features/playbook/PlaybookGroup.tsx`           |
| Card           | Playbooks > card                  | Assistant, goal, step count, first command; bookmark toggle                                        | `src/features/playbook/Card.tsx`                    |
| Start/Continue | Card > **Start** or **Continue**  | Opens `/agent?playbookId=…`; Continue when pinned to a session (warning when that session is gone) | `src/features/playbook/Card.tsx`, `launchTarget.ts` |
| Delete         | Card > trash > Delete             | Deletes the playbook                                                                               | `src/features/playbook/Card.tsx`                    |

### 1.7 Extensions (`/mcp-servers`)

Tabs **Tools** (default) and **Skills** (`?view=skills`). Source: `src/features/mcp-servers/MCPServerPage.tsx`.

#### 1.7.1 Tools tab

Source: `src/features/mcp-servers/MCPServerManagement.tsx`

| Name                    | Entry path                               | What it does                                                                                                                                                                                                                                                      | Source                                                                                                                                                          |
| ----------------------- | ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Add Extension           | Extensions > **Add Extension**           | Opens the add dialog                                                                                                                                                                                                                                              | `MCPServerManagement.tsx`                                                                                                                                       |
| Server card             | Extensions > Installed Extensions > card | Logo, transport, verification state, OAuth state                                                                                                                                                                                                                  | `components/ServerCard.tsx`                                                                                                                                     |
| Active                  | Card > switch                            | Activates or deactivates the server                                                                                                                                                                                                                               | `components/ServerCard.tsx`                                                                                                                                     |
| Tools                   | Card > **Tools**                         | Read-only list of tools with descriptions and schemas                                                                                                                                                                                                             | `components/ServerToolsModal.tsx`                                                                                                                               |
| Edit                    | Card > **Edit**                          | Opens the edit dialog                                                                                                                                                                                                                                             | `components/ServerCard.tsx`                                                                                                                                     |
| Retry                   | Card > **Retry** (conditional: error)    | Probes the server again                                                                                                                                                                                                                                           | `components/ServerCard.tsx`                                                                                                                                     |
| Open App Wizard         | Card (conditional: runtime error)        | Opens a draft chat with the App Wizard assistant                                                                                                                                                                                                                  | `components/ServerCard.tsx`                                                                                                                                     |
| Authorize / Disconnect  | Card (conditional: OAuth)                | Starts or revokes OAuth                                                                                                                                                                                                                                           | `components/ServerCard.tsx`                                                                                                                                     |
| Delete                  | Card > Delete > Delete                   | Removes the server                                                                                                                                                                                                                                                | `components/ServerCard.tsx`                                                                                                                                     |
| Load more               | Extensions > Load more extensions        | Next page                                                                                                                                                                                                                                                         | `MCPServerManagement.tsx`                                                                                                                                       |
| Recommended             | Extensions > Recommended Extensions      | Search, category chips, preset cards; zero-config presets install in one click, others open the install dialog; **Retry** on registry failure                                                                                                                     | `components/RecommendedPresets.tsx`                                                                                                                             |
| Add/Edit/Install dialog | Add, Edit, or preset card                | Name, Description, Logo URL, Transport (stdio / HTTP). stdio: Command, Arguments, Environment Variables. HTTP: URL, required config, auth None / Bearer / OAuth 2.1 (Client ID, Secret, Discovery URL or endpoints, Scopes, PKCE), Advanced (Custom Headers, SSE) | `MCPServerDialog.tsx`, `StdioForm.tsx`, `EnvVarsForm.tsx`, `HttpForm.tsx`, `TransportAdvancedFields.tsx`, `PresetServerSummary.tsx`, `PresetVariableFields.tsx` |

Paths without a folder are under `src/features/mcp-servers/` (dialog parts under its `components/`).

#### 1.7.2 Skills tab

Source: `src/features/skills/SkillsManagementPanel.tsx`

| Name                | Entry path                           | What it does                                                             | Source                                                    |
| ------------------- | ------------------------------------ | ------------------------------------------------------------------------ | --------------------------------------------------------- |
| Refresh             | Skills > Refresh                     | Reloads skills                                                           | `SkillsManagementPanel.tsx`                               |
| Installed skills    | Skills > View installed skills       | "Installed Skills ({{count}})": delete user skills; system skills locked | `src/features/settings/components/SkillsListModal.tsx`    |
| Skills folders      | Skills > System / User cards         | Open the folder in the OS                                                | `SkillsManagementPanel.tsx`                               |
| Reset user skills   | Skills > Reset user skills           | Confirm, then removes user skills                                        | `SkillsManagementPanel.tsx`                               |
| Install by drop     | Skills > drop `.skill`/`.zip`/folder | Installs locally                                                         | `src/features/skills/hooks/useSkillsManagementPanel.ts`   |
| Install from GitHub | Skills > repository URL > Install    | Installs from a GitHub repository                                        | `SkillsManagementPanel.tsx`                               |
| Conflict dialog     | Install (conditional: name conflict) | Choose user conflicts to replace; system conflicts skipped               | `src/features/skills/components/SkillsConflictDialog.tsx` |

### 1.8 Knowledge (`/knowledge`)

| Name             | Entry path                                        | What it does                                                                                                                         | Source                                                                                                |
| ---------------- | ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| View switcher    | Knowledge > Split / Graph / Cards                 | Changes the layout (not saved)                                                                                                       | `src/features/knowledge/KnowledgePage.tsx`                                                            |
| Refresh          | Knowledge > Refresh                               | Reloads                                                                                                                              | `KnowledgePage.tsx`                                                                                   |
| Search           | Knowledge > "Search content, tags, or source..."  | Filters chunks                                                                                                                       | `KnowledgePage.tsx`                                                                                   |
| Assistant filter | Knowledge > assistant select                      | All assistants or one                                                                                                                | `KnowledgePage.tsx`                                                                                   |
| Graph            | Knowledge > graph                                 | Pan, drag nodes, zoom; click to focus 1-hop; hover peek; **Zoom In/Out**, **Reset View**, **Reheat Physics**                         | `components/graph/KnowledgeNetworkCanvas.tsx`, `useCanvasInteraction.ts`, `KnowledgeNodePeekCard.tsx` |
| Entity filter    | Split > selected entity > Filter                  | Shows only linked chunks; **Inspect Entity**                                                                                         | `KnowledgePage.tsx`                                                                                   |
| Chunk list       | Split > Curated Stream; Cards > Knowledge Browser | Chunk cards with **Load more**                                                                                                       | `KnowledgePage.tsx`                                                                                   |
| Inspector        | Click a chunk or entity                           | Chunk: content, source, linked entities, relationships, **Delete permanently**. Entity: description, related entities, linked chunks | `components/KnowledgeInspectorSheet.tsx`                                                              |

Paths in `components/` are under `src/features/knowledge/`.

### 1.9 Scheduled Tasks (`/scheduled-tasks`)

#### 1.9.1 Task list

| Name              | Entry path                                                                    | What it does                                                                                                          | Source                                                |
| ----------------- | ----------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| New task          | Scheduled Tasks > **New task**                                                | Opens the task dialog                                                                                                 | `src/features/scheduled-tasks/ScheduledTasksPage.tsx` |
| Starter templates | Scheduled Tasks (conditional: zero tasks) > Use template                      | Three templates (knowledge distillation, PC health audit, website briefing) prefill the dialog; **Create blank task** | `components/ScheduledTaskRow.tsx`                     |
| Morning Briefing  | Scheduled Tasks (conditional: zero tasks) > Open Morning Briefing Walkthrough | §1.9.3                                                                                                                | `components/ScheduledTaskRow.tsx`                     |
| Summary           | Scheduled Tasks > Total / Enabled                                             | Counts                                                                                                                | `ScheduledTasksPage.tsx`                              |
| Task row          | Scheduled Tasks > row                                                         | Enable switch; schedule, mode, next run, workspace; **Edit Task**; **Delete Task** with confirm                       | `components/ScheduledTaskRow.tsx`                     |

#### 1.9.2 Task dialog

Source: `src/features/scheduled-tasks/components/ScheduledTaskModal.tsx`

| Name           | What it does                                                                                                            | Source                                                    |
| -------------- | ----------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| Task name      | Free text                                                                                                               | `ScheduledTaskModal.tsx`                                  |
| Assistant      | Select                                                                                                                  | `ScheduledTaskModal.tsx`                                  |
| Schedule       | Every N minutes / hours, Daily, Weekly, Monthly with time, weekday, day; or custom cron (**Switch to simple schedule**) | `components/ScheduleBuilder.tsx`                          |
| Workspace      | **Browse**, drop a folder, clear                                                                                        | `ScheduledTaskModal.tsx`, `hooks/useWorkspaceDropZone.ts` |
| Message        | Textarea with `@playbook:`, `@skill:`, `@file:` completion                                                              | `components/MentionTextarea.tsx`                          |
| Execution mode | Normal / YOLO / Full Auto, with a warning for Full Auto                                                                 | `ScheduledTaskModal.tsx`                                  |
| Reset planning | "Reset planning state before run" switch                                                                                | `ScheduledTaskModal.tsx`                                  |

#### 1.9.3 Morning Briefing walkthrough

Source: `src/features/recipes/components/MorningBriefingWalkthroughDialog.tsx`. Reached from the Chat start screen recipe card or the empty Scheduled Tasks page.

Steps: AI model check (**Go to Settings**), runtime check (**Recheck**, **App Wizard**), choose extensions (Hacker News, Yahoo Finance), preview. **Auto Setup & Receive Briefing Now** installs the extensions, creates the assistant and a daily 09:00 task, and opens a draft chat that runs the briefing.

### 1.10 Settings (`/settings`)

#### 1.10.1 Page shell

Changes are held as a draft until **Save Changes**, except UI size, which applies at once. The active tab is in `?tab=`. Source: `src/features/settings/SettingsPage.tsx`, `hooks/useSettingsPageController.ts`, `hooks/useSettingsForm.ts`.

| Name                     | Entry path                          | What it does                                                                     | Source                               |
| ------------------------ | ----------------------------------- | -------------------------------------------------------------------------------- | ------------------------------------ |
| Version                  | Settings > header                   | "LibrAgent v{version}"                                                           | `SettingsPage.tsx`                   |
| Changed badge / tab dots | Settings > header, tab bar          | "{{count}} sections changed"; amber dot per changed tab                          | `SettingsPage.tsx`                   |
| Restart required         | Settings > header; toast after save | Shown when a startup-only setting changed; **Restart now**                       | `hooks/restart-required-settings.ts` |
| Discard                  | Settings > Discard                  | Confirm, then drops the draft                                                    | `SettingsPage.tsx`                   |
| Close                    | Settings > Close                    | Goes back; with unsaved changes asks Cancel / Discard and Leave / Save and Leave | `SettingsPage.tsx`                   |
| Save Changes             | Settings > Save Changes             | Saves all settings and applies the language                                      | `hooks/useSettingsPageController.ts` |

Paths in §1.10 are under `src/features/settings/` unless they start with `src/`.

#### 1.10.2 General (`?tab=general`)

Source: `tabs/GeneralTab.tsx`

| Name                          | What it does                                                             | Source                            |
| ----------------------------- | ------------------------------------------------------------------------ | --------------------------------- |
| Language                      | English, 한국어, 简体中文, 日本語, Français, Español, Deutsch, Português | `tabs/GeneralTab.tsx`             |
| Color Theme                   | Neutral, Libr Amber, Iris Violet, Ocean Blue, Forest Sage                | `components/ColorThemePicker.tsx` |
| Font Family                   | Pretendard, Inter, NanumSquare Neo, D2Coding                             | `tabs/GeneralTab.tsx`             |
| UI size                       | 90/100/110/125/150 %, applied at once; **Reset to 100%**                 | `tabs/GeneralTab.tsx`             |
| Message Layout                | Document or Bubble                                                       | `tabs/GeneralTab.tsx`             |
| Tool Detail Level             | Simple or Detailed                                                       | `tabs/GeneralTab.tsx`             |
| Initial Response Speed Format | Time to first response, or tokens per second                             | `tabs/GeneralTab.tsx`             |
| Show feature tips             | Tip cards on/off (§1.2.4)                                                | `tabs/GeneralTab.tsx`             |
| Show Token Speed              | Shows tok/s in metrics                                                   | `tabs/GeneralTab.tsx`             |
| Compact Metrics               | Compact token metrics                                                    | `tabs/GeneralTab.tsx`             |

#### 1.10.3 AI & Models (`?tab=ai-models`)

Source: `tabs/AIModelsTab.tsx`

| Name                    | Entry path                                    | What it does                                                                                                                                    | Source                                                                        |
| ----------------------- | --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Default AI Model        | Model Preferences                             | Provider and model for new sessions; **Refresh models**                                                                                         | `tabs/AIModelsTab.tsx`, `src/features/agent/components/AgentModelPicker.tsx`  |
| Backup AI Model         | Model Preferences                             | Used when the default model fails after all retries                                                                                             | `tabs/AIModelsTab.tsx`                                                        |
| Thinking Depth          | Model Preferences                             | Off / Low / Medium / High / Auto                                                                                                                | `components/ThinkingEffortSection.tsx`                                        |
| Override Temperature    | Model Preferences                             | When on, **Temperature** 0–2 (default 0.7) is sent                                                                                              | `tabs/AIModelsTab.tsx`                                                        |
| Sub-Agent Model Routing | Sub-Agent Model Routing > low / normal / high | Model per complexity level for sub-agents; **Use default**                                                                                      | `tabs/AIModelsTab.tsx`                                                        |
| Provider API Keys       | Provider API Keys > provider card             | API key (show/hide) for OpenAI, Anthropic, Google Gemini, Ollama, Groq, Fireworks AI, Cerebras, OpenRouter; Base URL for OpenAI and Ollama only | `components/ProviderCard.tsx`                                                 |
| Custom OpenAI Providers | Custom OpenAI Providers > Add                 | Display name, Base URL, optional API key, optional manual model list; remove with confirm                                                       | `components/CustomProviderCard.tsx`, `src/lib/ai-service/custom-providers.ts` |

#### 1.10.4 Chat Interface (`?tab=chat-interface`)

Source: `tabs/ChatInterfaceTab.tsx`

| Name                        | What it does                                             |
| --------------------------- | -------------------------------------------------------- |
| Context Management Strategy | "Recent Messages Only" or "Auto-Summarize (Recommended)" |
| Max Output Tokens (Default) | 1K–128K presets, default 8K                              |
| Message Window Size         | Conditional: Recent Messages Only. Default 20            |
| Max Input Context           | Conditional: Auto-Summarize. 8K–4M, default 48K          |
| Tool Calls Visible Count    | 1–20, default 4                                          |
| Code Diff Context Lines     | 1–10, default 3                                          |

#### 1.10.5 System (`?tab=system`)

| Name                                  | Section                 | What it does                                                                                                                        | Source                                                           |
| ------------------------------------- | ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| Max File Upload Size (MB)             | File & Workspace        | 10–500 MB presets                                                                                                                   | `tabs/SystemTab.tsx`                                             |
| Your Chrome                           | Agent browser           | Extension bridge status, **Install from Chrome Web Store**, **Refresh status**, advanced info (folder path, port, token hint, mode) | `components/ExtensionBridgeSettingsSection.tsx`                  |
| Clear agent browser data              | Agent browser           | Confirm, then deletes the built-in agent browser's cookies and logins                                                               | `components/AgentBrowserDataSettingsSection.tsx`                 |
| Prevent sleep while app is running    | Background Tasks        | Switch, default on                                                                                                                  | `components/system-performance/SystemBackgroundTasksSection.tsx` |
| Search Index Frequency (Min)          | Background Tasks        | Default 5; restart required                                                                                                         | same                                                             |
| Web Action Timeout (Sec)              | Background Tasks        | Default 30; restart required                                                                                                        | same                                                             |
| Extension Discovery Timeout (Sec)     | Background Tasks        | 10–120, default 30                                                                                                                  | same                                                             |
| MCP Tool Execution Timeout (Sec)      | Background Tasks        | 0 = no limit                                                                                                                        | same                                                             |
| Keep App Logs (Days)                  | Background Tasks        | Default 30; 0 = no limit                                                                                                            | same                                                             |
| Scheduled Task Minimum Interval (Min) | Automation Rules        | 0 = no limit                                                                                                                        | `components/system-performance/SystemAutomationSection.tsx`      |
| HTTP Server Port                      | Network                 | Default 3030; restart required                                                                                                      | `components/system-performance/SystemNetworkSection.tsx`         |
| Expose HTTP Server                    | Network                 | Local only (127.0.0.1) or network (0.0.0.0); restart required                                                                       | same                                                             |
| Session API Keys                      | Session API Keys        | List issued keys, refresh, revoke with confirm                                                                                      | `components/SessionApiKeysSection.tsx`                           |
| Open Migration Dashboard              | Backup & Data Migration | Opens `/settings/migration` (§1.11)                                                                                                 | `tabs/SystemTab.tsx`                                             |

#### 1.10.6 Advanced (`?tab=advanced`)

| Name                             | Section                   | What it does                                                                                                                                                          | Source                                                         |
| -------------------------------- | ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Max Retry Attempts / Retry Delay | Request Reliability       | 0–5 retries; 1/3/5/10 s                                                                                                                                               | `tabs/advanced/AdvancedRequestReliabilitySection.tsx`          |
| Loop and wait limits             | Runtime controls          | Loop Prevention Threshold, Hard Break Offset, Tool Wait Duration, Wait Guide Count, Wait Break Count, Thinking Loop Min Pattern Length, Thinking Loop Min Repetitions | `tabs/advanced/AdvancedRuntimeControlsSection.tsx`             |
| Session Branching / Child Limit  | Runtime controls          | Max child depth and direct children; 0 = unlimited                                                                                                                    | same                                                           |
| Concurrency limits               | Performance & Concurrency | Max concurrent / suspended agent sessions and shell processes; restart required                                                                                       | `tabs/advanced/AdvancedPerformanceSection.tsx`                 |
| Recent Media Payload Messages    | Performance & Concurrency | How many recent media messages keep full data in requests                                                                                                             | same                                                           |
| Tool Result Inline Limit (KB)    | Performance & Concurrency | 4–256, default 16; larger output goes to a workspace file                                                                                                             | same                                                           |
| Shell Isolation Level            | Shell isolation           | Basic / Medium / High (OS sandbox)                                                                                                                                    | `tabs/advanced/AdvancedShellIsolationSection.tsx`              |
| Shell runtime bootstrap          | Shell isolation           | Switch, default off                                                                                                                                                   | same                                                           |
| Linked git worktrees             | Shell isolation           | Switch, default on                                                                                                                                                    | same                                                           |
| About                            | About                     | Version, update status, **Check for Updates**, **Install Update**                                                                                                     | `components/AboutSection.tsx`, `src/context/UpdateContext.tsx` |
| Clear All Sessions & Workspace   | Danger Zone               | Confirm, then deletes all sessions, messages, and workspaces                                                                                                          | `components/DangerZoneSettings.tsx`                            |
| Reset All Data & Settings        | Danger Zone               | Type `RESET` to confirm; factory reset                                                                                                                                | `components/DangerZoneSettings.tsx`                            |

#### 1.10.7 Experimental (`?tab=experimental`)

Source: `tabs/ExperimentalTab.tsx`

| Name                        | What it does                                                   |
| --------------------------- | -------------------------------------------------------------- |
| Inline Audio Attachments    | Sends full audio data to the model; off sends a file reference |
| Tool-loop recovery (legacy) | On: loop warnings in tool results. Off: resample, then break   |
| Max resample retries        | Conditional: legacy off. 0–20, default 2                       |

#### 1.10.8 Dev (`?tab=dev`) — dev-only

Shown only when `import.meta.env.DEV` is true; production ignores `?tab=dev`. Source: `tabs/DevTab.tsx`.

| Name             | What it does                                             |
| ---------------- | -------------------------------------------------------- |
| Provider         | Select a built-in provider (shows whether a key is set)  |
| Run sampleText() | Sends a test prompt; shows output, model, tokens, time   |
| Run compact()    | Runs compaction on sample messages and shows the summary |

#### 1.10.9 LLM providers and models

| Provider               | Settings fields                        | Model list                                                            |
| ---------------------- | -------------------------------------- | --------------------------------------------------------------------- |
| OpenAI                 | API key, Base URL                      | Live fetch with a key; else static catalog                            |
| Anthropic              | API key                                | Live fetch with a key; else static catalog                            |
| Google Gemini          | API key                                | Live fetch with a key; else static catalog                            |
| Ollama                 | API key, Base URL                      | Always live fetch                                                     |
| Groq                   | API key                                | Live fetch with a key; else static catalog                            |
| Fireworks AI           | API key                                | Live fetch with a key; else static catalog                            |
| Cerebras               | API key                                | Live fetch with a key; else static catalog                            |
| OpenRouter             | API key                                | Always live fetch                                                     |
| Custom (`custom:<id>`) | Name, Base URL, API key, manual models | Live `/v1/models` when a Base URL is set, merged with the manual list |

Sources: provider enum `src/lib/ai-service/types.ts`; static catalog `src/config/llm-config.json`; list resolution `src/context/ModelProvider.tsx`; fetch rules `src/lib/ai-service/model-fetch-policy.ts`; model cache `src/lib/ai-service/model-cache-storage.ts`.

Generation options are global, not per provider: Temperature (§1.10.3), Max Output Tokens and Max Input Context (§1.10.4), retries (§1.10.6), Thinking Depth (§1.10.3; also in the chat status bar and draft model picker).

Thinking Depth maps per provider in `src/lib/ai-service/thinking-effort-mapping.ts`:

- OpenAI, Fireworks, Cerebras, OpenRouter, Ollama: `reasoningEffort` (`auto` is sent as `medium`).
- Groq: reasoning on, no effort level.
- Anthropic: any level other than Off turns on extended thinking.
- Gemini: thinking budget low 1024, medium 8192, high 24576, auto -1.

Per-session model: chat status bar (§1.3.5) and draft model picker (§1.3.2). Execution mode is per session (§1.3.5, §1.3.6); it has no Settings control.

### 1.11 Data Migration (`/settings/migration`)

Source: `src/features/migration/MigrationPage.tsx`, `hooks/useMigrationPage.ts`, `components/*`

| Name             | Entry path                                   | What it does                                                                                                             | Source                                    |
| ---------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------- |
| Back             | Migration > back arrow                       | Opens `/settings`                                                                                                        | `MigrationPage.tsx`                       |
| Export           | Migration > Export Settings                  | Include sensitive data (forces a password), choose folder, **Run Export** to `libragent-backup-<ts>.libragent-migration` | `components/MigrationIdleCards.tsx`       |
| Import           | Migration > Import Settings                  | Select a `.libragent-migration` file and validate; current database is backed up first                                   | `components/MigrationIdleCards.tsx`       |
| Password dialogs | Export with sensitive data; encrypted import | Set password (min 4, confirm) or enter password                                                                          | `components/MigrationPasswordDialogs.tsx` |
| Progress         | During export / import                       | Phase title and percentage                                                                                               | `components/MigrationLoadingCard.tsx`     |
| Import preview   | After validation                             | Compatibility, version, size, sections; conflict mode Skip / Overwrite / Merge; **Run Import**                           | `components/MigrationPreviewCard.tsx`     |
| Done             | After export / import                        | Result table; **Re-authenticate and verify MCP servers**; **Back to migration home**                                     | `components/MigrationCompleteCard.tsx`    |

Paths in `components/` and `hooks/` are under `src/features/migration/`.

### 1.12 Dev-only, hidden, and conditional items

| Item                                     | Kind        | Note                                                                     | Source                                                         |
| ---------------------------------------- | ----------- | ------------------------------------------------------------------------ | -------------------------------------------------------------- |
| Settings > Dev tab                       | dev-only    | `import.meta.env.DEV`                                                    | `src/features/settings/tabs/DevTab.tsx`                        |
| Update auto-check                        | dev-only    | Skipped in dev builds; manual check still in Settings > Advanced > About | `src/context/UpdateContext.tsx`                                |
| `/history/search`                        | hidden      | Route with no link; same screen as `/history`                            | `src/app/App.tsx`                                              |
| App control bridge                       | hidden      | Backend events only                                                      | `src/features/app-control/useAppControlBridge.tsx`             |
| Session API pairing dialog               | hidden      | Event-driven; no navigation entry                                        | `src/features/agent/components/SessionApiPairingDialog.tsx`    |
| Slash commands and `@` mentions          | hidden      | Found only by typing                                                     | `src/features/agent/hooks/useInputToken.ts`                    |
| Agent Hub URL                            | hidden      | `agentHubUrl` setting and locale strings exist; no input is rendered     | `src/lib/services/settings-service.ts`                         |
| Provider options without UI              | hidden      | Gemini `safetySettings`; OpenAI `use3rdParty`, `customModelId`           | `src/lib/services/settings-service.ts`                         |
| Message search                           | hidden      | `searchMessages` wrapper exists; no screen calls it                      | `src/lib/backend/messages.ts`                                  |
| Feature tips                             | conditional | `display.showFeatureTips` and per-tip dismissal                          | `src/features/spotlight/*`                                     |
| Starter templates, Morning Briefing link | conditional | Only while the Scheduled Tasks list is empty                             | `src/features/scheduled-tasks/components/ScheduledTaskRow.tsx` |
| Onboarding banners                       | conditional | No provider configured; npx not ready                                    | `src/features/agent/AgentChatStartView.tsx`                    |
| Isolation Settings in draft              | conditional | Hidden while a workspace override is set                                 | `src/features/agent/components/DraftCapabilitiesSection.tsx`   |

## 2. Classification and priority

Added by #2750. Every §1 inventory item gets exactly one class, and every direct or adapted item gets one priority.

Classes:

- **direct** — works the same in a terminal: text, lists, forms, toggles, confirmations.
- **adapted** — needs a terminal-specific form; the note says what that form is.
- **excluded** — not offered in the TUI; the note states the reason. Parity is the default: exclusion is only for things a terminal cannot render meaningfully (window chrome, fonts, canvas).

Priorities:

- **P0** — needed for the first release: a full chat turn (send, stream, tool approval, askUser, secret prompt) plus `/new` and `/resume`, and the configuration required to send a first message.
- **P1** — also required for phase 1: the surfaces a day-to-day user manages (session management, assistants, playbooks, extensions, skills, knowledge text flows, scheduled tasks, settings).
- **P2** — not required for phase 1: convenience, visual-only, or deep-dive surfaces with a workable fallback.

Excluded items are listed with reasons in place; the rollup is §2.15.

### 2.1 Routes and coverage (§1.1)

| Route / panel          | Class    | Priority | Terminal form                                  |
| ---------------------- | -------- | -------- | ---------------------------------------------- |
| `/`                    | direct   | P0       | TUI starts at the chat screen                  |
| `/agent`               | direct   | P0       | default screen; `/new` starts a draft          |
| `/agent/draft`         | direct   | P0       | draft state before first send                  |
| `/agent/:sessionId`    | direct   | P0       | `/resume <id>` or picker                       |
| `/assistants`          | direct   | P1       | `/assistants`                                  |
| `/playbooks`           | direct   | P1       | `/playbooks`                                   |
| `/history`             | direct   | P1       | `/history`                                     |
| `/history/search`      | excluded | —        | hidden duplicate route with no link in the GUI |
| `/org`                 | direct   | P2       | `/org`                                         |
| `/settings`            | direct   | P1       | `/settings`                                    |
| `/settings/migration`  | direct   | P2       | `/settings migration`                          |
| `/mcp-servers`         | direct   | P1       | `/extensions`                                  |
| `/knowledge`           | direct   | P1       | `/knowledge`                                   |
| `/scheduled-tasks`     | direct   | P1       | `/tasks`                                       |
| Settings tabs 1.10.2–8 | —        | —        | classified in §2.13 (they are §1.10 items)     |

### 2.2 App shell (§1.2)

| Item (§1 ref)                         | Class    | Priority | Terminal form                                                      |
| ------------------------------------- | -------- | -------- | ------------------------------------------------------------------ | --------- | --------- |
| Sidebar nav entries (§1.2.1)          | adapted  | see rows | the sidebar becomes slash commands                                 |
| History (§1.2.1)                      | adapted  | P1       | `/history`                                                         |
| Bookmarked (§1.2.1)                   | adapted  | P1       | `/history --bookmarked`                                            |
| Chat (§1.2.1)                         | adapted  | P0       | `/new`                                                             |
| Knowledge (§1.2.1)                    | adapted  | P1       | `/knowledge`                                                       |
| Assistants (§1.2.1)                   | adapted  | P1       | `/assistants`                                                      |
| Playbooks (§1.2.1)                    | adapted  | P1       | `/playbooks`                                                       |
| Extensions (§1.2.1)                   | adapted  | P1       | `/extensions`                                                      |
| Org (§1.2.1)                          | adapted  | P2       | `/org`                                                             |
| Scheduled Tasks (§1.2.1)              | adapted  | P1       | `/tasks`                                                           |
| Recent Sessions (§1.2.1)              | adapted  | P0       | `/resume` picker shows the same list with status                   |
| Settings (§1.2.1)                     | adapted  | P1       | `/settings`; version shown in the status line                      |
| Collapse sidebar (§1.2.1)             | excluded | —        | no persistent sidebar in a terminal                                |
| Sidebar toggle (§1.2.2)               | excluded | —        | window chrome; nothing to collapse                                 |
| Notifications bell (§1.2.2)           | adapted  | P1       | `/attention` list plus a status-line badge count                   |
| Theme toggle (§1.2.2)                 | excluded | —        | light/dark is the terminal emulator's job                          |
| Agent Pairing Request dialog (§1.2.3) | adapted  | P1       | blocking prompt with pairing code, Approve / Reject (Esc rejects)  |
| Update toast (§1.2.3)                 | adapted  | P2       | status-line notice; install runs the updater and asks to quit      |
| Toasts (§1.2.3)                       | adapted  | P0       | inline notification lines in the transcript                        |
| UI scale shortcuts (§1.2.3)           | excluded | —        | font size is the terminal emulator's job                           |
| Agent panel shortcuts (§1.2.3)        | adapted  | P1       | `/panel workspace                                                  | processes | planning` |
| App control bridge (§1.2.3)           | excluded | —        | hidden automation hook with no user surface; backend keeps working |
| "What's new" card (§1.2.4)            | excluded | —        | GUI tip cards; terminal users read the changelog                   |
| "Tip" card (§1.2.4)                   | excluded | —        | as above                                                           |
| "Tip · title" link (§1.2.4)           | excluded | —        | as above                                                           |
| Tip CTA targets (§1.2.4)              | excluded | —        | tips are not shown in the TUI                                      |

### 2.3 Chat: start and draft (§1.3.1–§1.3.2)

| Item (§1 ref)                         | Class    | Priority | Terminal form                                                 |
| ------------------------------------- | -------- | -------- | ------------------------------------------------------------- |
| AI model banner (§1.3.1)              | adapted  | P0       | start-screen warning line; `/settings ai-models` to configure |
| Runtime banner (§1.3.1)               | adapted  | P1       | warning line with `/setup-wizard` and recheck action          |
| Feature tips (§1.3.1)                 | excluded | —        | see §1.2.4 tips                                               |
| Morning Briefing recipe card (§1.3.1) | adapted  | P2       | `/morning-briefing` starts the walkthrough                    |
| Built-in Assistants (§1.3.1)          | direct   | P0       | `/new` lists assistants as a picker                           |
| My Assistants (§1.3.1)                | direct   | P0       | same picker                                                   |
| Manage Assistants (§1.3.1)            | adapted  | P1       | `/assistants`                                                 |
| Empty / error state (§1.3.1)          | direct   | P0       | plain error text with a create action                         |
| Playbook start (§1.3.1)               | adapted  | P1       | `/playbooks` card action                                      |
| Draft header (§1.3.2)                 | direct   | P0       | header line "[assistant] · New Session"                       |
| Capabilities (§1.3.2)                 | adapted  | P1       | text badge list under the header                              |
| Add tools (§1.3.2)                    | adapted  | P1       | opens the assistant editor Tools tab                          |
| Model and thinking (§1.3.2)           | adapted  | P0       | `/model` and `/thinking` pickers for the draft                |
| Isolation Settings (§1.3.2)           | adapted  | P2       | `/docker` toggle plus image argument                          |
| Workspace by drop (§1.3.2)            | adapted  | P1       | `/workspace <path>` argument (no drag-and-drop in a terminal) |
| Draft composer (§1.3.2)               | direct   | P0       | the input line                                                |
| Create on send (§1.3.2)               | direct   | P0       | identical backend flow                                        |
| No-provider toast (§1.3.2)            | direct   | P0       | inline error with settings action                             |
| Docker error dialog (§1.3.2)          | adapted  | P2       | prompt with the same four choices                             |

### 2.4 Chat: session screen and header (§1.3.3–§1.3.4)

| Item (§1 ref)                      | Class   | Priority | Terminal form                                                    |
| ---------------------------------- | ------- | -------- | ---------------------------------------------------------------- | -------------------------------------------------- |
| Loading overlay (§1.3.3)           | direct  | P0       | status lines while the session starts                            |
| Load error (§1.3.3)                | direct  | P0       | inline error text                                                |
| Playbook auto-start (§1.3.3)       | adapted | P1       | `/playbooks` start action prints the same strip                  |
| MCP discovery toasts (§1.3.3)      | direct  | P1       | inline lines per server                                          |
| Keep-alive (§1.3.3)                | direct  | P0       | no client-side work: the shared backend runs sessions regardless |
| Title (§1.3.4)                     | direct  | P0       | header line                                                      |
| Rename session (§1.3.4)            | direct  | P1       | `/rename <name>`                                                 |
| Bookmark (§1.3.4)                  | direct  | P1       | `/bookmark`                                                      |
| Export session (§1.3.4)            | adapted | P1       | `/export md                                                      | atif <path>` writes the file instead of a download |
| Copy pairing instructions (§1.3.4) | adapted | P2       | `/pairing` prints the instructions (terminal clipboard varies)   |
| Panel update badges (§1.3.4)       | adapted | P1       | status-line hint "planning updated"                              |
| Toggle agent panels (§1.3.4)       | adapted | P1       | `/panel ...`                                                     |
| Session files (§1.3.4)             | adapted | P1       | `/files` lists; path shown; open with default app on request     |

### 2.5 Chat: status bar (§1.3.5)

| Item (§1 ref)                     | Class   | Priority | Terminal form                                           |
| --------------------------------- | ------- | -------- | ------------------------------------------------------- | ---- | --------------------------------- | ---- | ----- |
| Workflow banner (§1.3.5)          | direct  | P0       | persistent status line with Retry / Continue keys       |
| Session model (§1.3.5)            | adapted | P0       | `/model` picker; same idle-only rule and refresh action |
| Sub-session batch update (§1.3.5) | adapted | P2       | confirm prompt with the same three choices              |
| Thinking effort (§1.3.5)          | direct  | P1       | `/thinking off                                          | low  | medium                            | high | auto` |
| Execution mode (§1.3.5)           | direct  | P0       | `/permission normal                                     | yolo | unsafe` (already a slash command) |
| Token metrics (§1.3.5)            | adapted | P2       | `/stats` prints the same numbers                        |
| Tools count (§1.3.5)              | adapted | P1       | `/tools` list with badges and schemas                   |
| Retry failed MCP (§1.3.5)         | direct  | P1       | `/mcp retry`                                            |

### 2.6 Chat: composer (§1.3.6)

| Item (§1 ref)               | Class   | Priority | Terminal form                                                           |
| --------------------------- | ------- | -------- | ----------------------------------------------------------------------- |
| Mode badge (§1.3.6)         | direct  | P0       | status-line text                                                        |
| Attached files bar (§1.3.6) | adapted | P0       | attached paths listed above the input; remove by number                 |
| Queued prompts (§1.3.6)     | direct  | P0       | queued list with cancel-by-number                                       |
| Input (§1.3.6)              | direct  | P0       | Enter sends, Shift+Enter newline, IME composition respected             |
| `@` mentions (§1.3.6)       | adapted | P1       | typing `@` opens the same picker list (skills, files, tools, playbooks) |
| `/clear` (§1.3.6)           | direct  | P0       | identical                                                               |
| `/reload` (§1.3.6)          | direct  | P0       | identical                                                               |
| `/permission …` (§1.3.6)    | direct  | P0       | identical                                                               |
| Attach file (§1.3.6)        | adapted | P0       | `/attach <path>` argument (no file picker)                              |
| Drop files (§1.3.6)         | adapted | P0       | same `/attach <path>` path                                              |
| Paste image (§1.3.6)        | adapted | P2       | terminals cannot paste image bytes; attach a saved file instead         |
| Cancel request (§1.3.6)     | direct  | P0       | Esc / Ctrl+C; skipped while a prompt is open                            |
| Resume (§1.3.6)             | direct  | P0       | key shown in the status line when paused                                |
| Oversize guard (§1.3.6)     | direct  | P0       | identical rejection message                                             |

### 2.7 Chat: messages (§1.3.7)

| Item (§1 ref)              | Class   | Priority | Terminal form                                                                                                                                                                             |
| -------------------------- | ------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Load older (§1.3.7)        | adapted | P0       | pager scrolls into history; loads 50 at a time on demand                                                                                                                                  |
| Jump to approvals (§1.3.7) | adapted | P0       | status-line badge "n awaiting approval"; approval prompt is inline                                                                                                                        |
| Scroll to latest (§1.3.7)  | adapted | P0       | pager key                                                                                                                                                                                 |
| Compact divider (§1.3.7)   | adapted | P1       | "Context compressed above" line; expand key shows the summary                                                                                                                             |
| Role label (§1.3.7)        | direct  | P0       | prefix line per message                                                                                                                                                                   |
| Copy message (§1.3.7)      | adapted | P1       | `/copy <n>` writes the text to a file (terminal clipboard varies)                                                                                                                         |
| Regenerate (§1.3.7)        | direct  | P1       | `/regenerate`                                                                                                                                                                             |
| Copy tool calls (§1.3.7)   | adapted | P2       | JSON dump to a file                                                                                                                                                                       |
| Export message (§1.3.7)    | adapted | P2       | session export covers it; per-message export later                                                                                                                                        |
| Error bubble (§1.3.7)      | direct  | P0       | error text with Try Again / settings actions                                                                                                                                              |
| Thinking (§1.3.7)          | adapted | P0       | collapsed "Thinking…" block, expand key, auto-expand while streaming                                                                                                                      |
| Rich content (§1.3.7)      | adapted | P0       | Markdown and code rendered in-terminal; Mermaid becomes source plus file link; images, audio, and video become paths with an open-externally action; resource links become openable paths |

### 2.8 Chat: tool cards (§1.3.8)

| Item (§1 ref)                     | Class   | Priority | Terminal form                                                                                      |
| --------------------------------- | ------- | -------- | -------------------------------------------------------------------------------------------------- |
| Tool group (§1.3.8)               | direct  | P0       | "Tool Executions (n)" block with status badges                                                     |
| Tool call row (§1.3.8)            | direct  | P0       | expandable parameters and result, same detail level                                                |
| MCP UI resource (§1.3.8)          | adapted | P2       | action list rendered as text choices; interactive form fallback                                    |
| Legacy form (§1.3.8)              | adapted | P2       | read-only text dump of the form (already disabled in GUI)                                          |
| File write card (§1.3.8)          | adapted | P0       | badge plus unified diff in text; preview as pager content                                          |
| String replace card (§1.3.8)      | direct  | P0       | diff text                                                                                          |
| Shell output card (§1.3.8)        | direct  | P0       | exit code, duration, stdout, stderr block                                                          |
| Report result card (§1.3.8)       | direct  | P0       | status, criteria, proof; deliverables listed as paths                                              |
| Deliverable file actions (§1.3.8) | adapted | P0       | Preview opens the pager; Open runs the OS open command; Download is a no-op (already a local path) |
| Sub-agent card (§1.3.8)           | adapted | P1       | state, task, result as text; `/resume <child-id>` opens it                                         |

### 2.9 Chat: in-chat prompts (§1.3.9)

| Item (§1 ref)            | Class   | Priority | Terminal form                                              |
| ------------------------ | ------- | -------- | ---------------------------------------------------------- |
| Tool approval (§1.3.9)   | direct  | P0       | blocking prompt with tool name and JSON; Approve / Reject  |
| Secret prompt (§1.3.9)   | direct  | P0       | masked password input; Esc cancels                         |
| Question prompt (§1.3.9) | direct  | P0       | list-based picker per question, same multi-select and text |
| Pairing request (§1.3.9) | adapted | P1       | see §2.2 (dialog becomes a prompt)                         |

### 2.10 Side panels (§1.3.10)

| Item (§1 ref)                | Class   | Priority | Terminal form                                                    |
| ---------------------------- | ------- | -------- | ---------------------------------------------------------------- |
| Workspace toolbar (§1.3.10)  | adapted | P1       | `/panel workspace` with open-explorer, open-terminal, refresh    |
| Workspace override (§1.3.10) | adapted | P1       | `/workspace <path>` and `/workspace clear`                       |
| File tree (§1.3.10)          | adapted | P1       | indented tree list; file click previews or opens externally      |
| Upload zone (§1.3.10)        | adapted | P1       | `/import <path>` into the workspace                              |
| File preview (§1.3.10)       | adapted | P1       | pager for text and Markdown; HTML source; others open externally |
| Processes (§1.3.10)          | adapted | P1       | `/panel processes` list with view, status, stop                  |
| Process output (§1.3.10)     | adapted | P1       | pager with stdout and stderr                                     |
| Planning (§1.3.10)           | adapted | P1       | `/panel planning` shows goal, tasks, scratchpad                  |
| Session schedules (§1.3.10)  | adapted | P1       | list with pause, resume, cancel actions                          |
| Panel attention (§1.3.10)    | adapted | P2       | status-line "planning updated" hints                             |

### 2.11 Session management (§1.4)

| Item (§1 ref)                     | Class   | Priority | Terminal form                                               |
| --------------------------------- | ------- | -------- | ----------------------------------------------------------- | -------------- |
| New session (§1.4.1)              | direct  | P0       | `/new`                                                      |
| From playbook (§1.4.1)            | adapted | P1       | `/playbooks` start                                          |
| From scheduled task (§1.4.1)      | direct  | P0       | backend behaviour, no user surface                          |
| Recent Sessions list (§1.4.2)     | adapted | P0       | `/resume` picker                                            |
| History page (§1.4.2)             | adapted | P1       | `/history`                                                  |
| Search (§1.4.2)                   | adapted | P1       | `/history search <text>`                                    |
| Sort (§1.4.2)                     | adapted | P1       | `/history --sort …`                                         |
| Status filter (§1.4.2)            | adapted | P1       | `/history --status …`                                       |
| Bookmarked filter (§1.4.2)        | adapted | P1       | `/history --bookmarked`                                     |
| Bookmarked strip (§1.4.2)         | adapted | P2       | the bookmarked filter covers it                             |
| Lineage filter (§1.4.2)           | adapted | P2       | `/history --lineage <id>`                                   |
| Tree badges (§1.4.2)              | adapted | P1       | text badges on rows                                         |
| Open from sidebar (§1.4.3)        | direct  | P0       | `/resume`                                                   |
| Resume from History (§1.4.3)      | adapted | P1       | row action in `/history`                                    |
| From notification (§1.4.3)        | adapted | P1       | `/attention` row action                                     |
| From Org (§1.4.3)                 | adapted | P2       | `/org` card action                                          |
| From sub-agent card (§1.4.3)      | adapted | P1       | `/resume <child-id>`                                        |
| Rename (§1.4.4)                   | direct  | P1       | `/rename`                                                   |
| Bookmark (§1.4.4)                 | direct  | P1       | `/bookmark`                                                 |
| Delete (§1.4.4)                   | direct  | P1       | `/delete` with confirm                                      |
| Delete with children (§1.4.4)     | adapted | P1       | confirm prompt offers both choices                          |
| Clear all sessions (§1.4.4)       | adapted | P2       | danger-zone action in `/settings advanced`                  |
| Markdown export (§1.4.5)          | adapted | P1       | `/export md <path>`                                         |
| ATIF export (§1.4.5)              | adapted | P1       | `/export atif <path>`                                       |
| Copy visible (§1.4.5)             | adapted | P1       | `/export visible <path>`                                    |
| Org bundle (§1.4.5)               | adapted | P2       | `/org export                                                | import <path>` |
| Sidebar status dot (§1.4.6)       | adapted | P0       | status column in every session list                         |
| History status badge (§1.4.6)     | adapted | P1       | same words as text badges                                   |
| Live updates (§1.4.6)             | direct  | P0       | the TUI consumes the same `agent:event` stream              |
| Attention list (§1.4.6)           | adapted | P1       | `/attention`                                                |
| Mark viewed (§1.4.6)              | direct  | P1       | automatic, identical triggers                               |
| Import Org (§1.4.7)               | adapted | P2       | `/org import <path>`                                        |
| Org refresh / search (§1.4.7)     | adapted | P2       | `/org` actions                                              |
| Org card (§1.4.7)                 | adapted | P2       | text card with counts and lineage list                      |
| Export Org (§1.4.7)               | adapted | P2       | `/org export`                                               |
| Delete org (§1.4.7)               | adapted | P2       | `/org delete` with confirm                                  |
| Resume root (§1.4.7)              | adapted | P2       | `/org` card action                                          |
| Workspace at creation (§1.4.8)    | adapted | P1       | `/new --workspace <path>`                                   |
| Override after creation (§1.4.8)  | adapted | P1       | `/workspace <path>`                                         |
| Scheduled task workspace (§1.4.8) | adapted | P1       | path field in the task form                                 |
| Backend commands (§1.4.9)         | —       | —        | reference table, no classification; TUI reuses the commands |

### 2.12 Feature pages (§1.5–§1.9)

| Item (§1 ref)                         | Class    | Priority | Terminal form                                              |
| ------------------------------------- | -------- | -------- | ---------------------------------------------------------- |
| Assistants search (§1.5)              | direct   | P1       | `/assistants <text>`                                       |
| Create (§1.5)                         | direct   | P1       | `/assistants new`                                          |
| Card expand (§1.5)                    | direct   | P1       | list row expands to prompt and badges                      |
| Edit (§1.5)                           | direct   | P1       | `/assistants edit <id>`                                    |
| Delete (§1.5)                         | direct   | P1       | confirm prompt                                             |
| Editor General (§1.5)                 | adapted  | P1       | form wizard: name, description, prompt (multi-line editor) |
| Editor Tools (§1.5)                   | adapted  | P1       | toggle and checkbox lists                                  |
| Editor Skills (§1.5)                  | adapted  | P1       | enable or disable list; add by path                        |
| Playbooks refresh (§1.6)              | direct   | P1       | `/playbooks` reload key                                    |
| Playbooks search (§1.6)               | direct   | P1       | filter text                                                |
| Playbooks display options (§1.6)      | adapted  | P2       | sort flag only; grouping is list-native                    |
| Playbooks group header (§1.6)         | excluded | —        | terminal lists are flat; sort covers ordering              |
| Playbooks card (§1.6)                 | direct   | P1       | list row with goal and steps                               |
| Playbooks Start / Continue (§1.6)     | direct   | P1       | row action                                                 |
| Playbooks delete (§1.6)               | direct   | P1       | confirm prompt                                             |
| Extensions Add Extension (§1.7.1)     | adapted  | P1       | form wizard (name, transport, env, headers)                |
| Extensions server card (§1.7.1)       | adapted  | P1       | text card: transport, verification, OAuth state            |
| Extensions Active switch (§1.7.1)     | direct   | P1       | `/extensions active <name>`                                |
| Extensions Tools list (§1.7.1)        | adapted  | P1       | `/extensions tools <name>` pager                           |
| Extensions Edit (§1.7.1)              | adapted  | P1       | same wizard prefilled                                      |
| Extensions Retry (§1.7.1)             | direct   | P1       | `/extensions retry <name>`                                 |
| Open App Wizard (§1.7.1)              | adapted  | P2       | creates the wizard draft chat, same as GUI                 |
| Authorize / Disconnect (§1.7.1)       | adapted  | P1       | starts the same OAuth flow; browser opens externally       |
| Extensions Delete (§1.7.1)            | direct   | P1       | confirm prompt                                             |
| Extensions Load more (§1.7.1)         | direct   | P1       | pager                                                      |
| Recommended presets (§1.7.1)          | adapted  | P1       | searchable preset list; one-click install stays            |
| Add / Edit / Install dialog (§1.7.1)  | adapted  | P1       | wizard pages instead of one modal                          |
| Skills refresh (§1.7.2)               | direct   | P1       | reload key                                                 |
| Installed skills (§1.7.2)             | adapted  | P1       | list with delete; system skills marked locked              |
| Skills folders (§1.7.2)               | adapted  | P2       | paths printed; open command on request                     |
| Reset user skills (§1.7.2)            | direct   | P1       | confirm prompt                                             |
| Install by drop (§1.7.2)              | adapted  | P1       | `/skills install <path>`                                   |
| Install from GitHub (§1.7.2)          | direct   | P1       | `/skills install <url>`                                    |
| Conflict dialog (§1.7.2)              | direct   | P1       | picker with the same choices                               |
| Knowledge view switcher (§1.8)        | excluded | —        | the terminal has one layout                                |
| Knowledge refresh (§1.8)              | direct   | P1       | reload key                                                 |
| Knowledge search (§1.8)               | direct   | P1       | `/knowledge search <text>`                                 |
| Knowledge assistant filter (§1.8)     | direct   | P1       | `/knowledge --assistant …`                                 |
| Knowledge graph (§1.8)                | adapted  | P2       | text citation list (neighbours, relationships); no canvas  |
| Knowledge entity filter (§1.8)        | adapted  | P1       | `/knowledge entity <id>` with inspect                      |
| Knowledge chunk list (§1.8)           | adapted  | P1       | pager with load-more                                       |
| Knowledge inspector (§1.8)            | adapted  | P1       | detail view; delete with confirm                           |
| Scheduled tasks New task (§1.9.1)     | direct   | P1       | `/tasks new`                                               |
| Starter templates (§1.9.1)            | adapted  | P2       | `/tasks template` picker                                   |
| Morning Briefing link (§1.9.1)        | adapted  | P2       | `/morning-briefing`                                        |
| Scheduled tasks summary (§1.9.1)      | direct   | P1       | header line                                                |
| Scheduled task row (§1.9.1)           | adapted  | P1       | row with enable, edit, delete actions                      |
| Task form fields (§1.9.2)             | direct   | P1       | form wizard: name, assistant, schedule, message, mode      |
| Schedule builder (§1.9.2)             | direct   | P1       | simple fields plus raw cron input                          |
| Workspace field (§1.9.2)              | adapted  | P1       | path argument                                              |
| Message field with mentions (§1.9.2)  | adapted  | P1       | same `@` picker                                            |
| Execution mode field (§1.9.2)         | direct   | P1       | choice list                                                |
| Reset planning switch (§1.9.2)        | direct   | P1       | toggle                                                     |
| Morning Briefing walkthrough (§1.9.3) | adapted  | P2       | scripted step-by-step prompts                              |

### 2.13 Settings (§1.10–§1.11)

| Item (§1 ref)                              | Class    | Priority | Terminal form                                               |
| ------------------------------------------ | -------- | -------- | ----------------------------------------------------------- |
| Settings page shell (§1.10.1)              | —        | —        | section-level rows below                                    |
| Version (§1.10.1)                          | direct   | P1       | `/settings` header                                          |
| Changed badge / tab dots (§1.10.1)         | adapted  | P1       | "n sections changed" line                                   |
| Restart required (§1.10.1)                 | direct   | P1       | same wording                                                |
| Discard (§1.10.1)                          | direct   | P1       | confirm prompt                                              |
| Close (§1.10.1)                            | direct   | P1       | same unsaved-changes choices                                |
| Save Changes (§1.10.1)                     | direct   | P1       | `/settings save`                                            |
| Language (§1.10.2)                         | direct   | P1       | choice list                                                 |
| Color Theme (§1.10.2)                      | excluded | —        | terminal colours belong to the emulator                     |
| Font Family (§1.10.2)                      | excluded | —        | terminal fonts belong to the emulator                       |
| UI size (§1.10.2)                          | excluded | —        | terminal font size belongs to the emulator                  |
| Message Layout (§1.10.2)                   | direct   | P2       | stored; the TUI always uses the document look               |
| Tool Detail Level (§1.10.2)                | direct   | P1       | choice list                                                 |
| Initial Response Speed Format (§1.10.2)    | direct   | P2       | choice list                                                 |
| Show feature tips (§1.10.2)                | direct   | P2       | stored for the GUI; the TUI shows no tips                   |
| Show Token Speed (§1.10.2)                 | direct   | P2       | toggle                                                      |
| Compact Metrics (§1.10.2)                  | direct   | P2       | toggle                                                      |
| Default AI Model (§1.10.3)                 | adapted  | P0       | `/model` picker with provider list and refresh              |
| Backup AI Model (§1.10.3)                  | adapted  | P1       | picker                                                      |
| Thinking Depth (§1.10.3)                   | direct   | P1       | choice list                                                 |
| Override Temperature (§1.10.3)             | direct   | P1       | toggle plus numeric input                                   |
| Sub-Agent Model Routing (§1.10.3)          | adapted  | P1       | three pickers plus Use default                              |
| Provider API Keys (§1.10.3)                | adapted  | P1       | masked secret inputs per provider                           |
| Custom OpenAI Providers (§1.10.3)          | adapted  | P1       | form wizard                                                 |
| Context Management Strategy (§1.10.4)      | direct   | P1       | choice list                                                 |
| Max Output Tokens (§1.10.4)                | direct   | P1       | preset list                                                 |
| Message Window Size (§1.10.4)              | direct   | P1       | numeric input                                               |
| Max Input Context (§1.10.4)                | direct   | P1       | preset list                                                 |
| Tool Calls Visible Count (§1.10.4)         | direct   | P1       | numeric input                                               |
| Code Diff Context Lines (§1.10.4)          | direct   | P1       | numeric input                                               |
| Max File Upload Size (§1.10.5)             | direct   | P1       | preset list                                                 |
| Your Chrome (§1.10.5)                      | adapted  | P2       | status lines; Web Store link opens the browser              |
| Clear agent browser data (§1.10.5)         | direct   | P2       | confirm prompt                                              |
| Prevent sleep (§1.10.5)                    | direct   | P1       | toggle                                                      |
| Search Index Frequency (§1.10.5)           | direct   | P1       | numeric input                                               |
| Web Action Timeout (§1.10.5)               | direct   | P1       | numeric input                                               |
| Extension Discovery Timeout (§1.10.5)      | direct   | P1       | numeric input                                               |
| MCP Tool Execution Timeout (§1.10.5)       | direct   | P1       | numeric input                                               |
| Keep App Logs (§1.10.5)                    | direct   | P1       | numeric input                                               |
| Scheduled Task Minimum Interval (§1.10.5)  | direct   | P1       | numeric input                                               |
| HTTP Server Port (§1.10.5)                 | direct   | P1       | numeric input                                               |
| Expose HTTP Server (§1.10.5)               | direct   | P1       | choice list                                                 |
| Session API Keys (§1.10.5)                 | adapted  | P1       | key list with refresh and revoke-confirm                    |
| Open Migration Dashboard (§1.10.5)         | direct   | P2       | `/settings migration`                                       |
| Max Retry Attempts / Retry Delay (§1.10.6) | direct   | P1       | numeric and preset inputs                                   |
| Loop and wait limits (§1.10.6)             | direct   | P1       | numeric inputs                                              |
| Session Branching / Child Limit (§1.10.6)  | direct   | P1       | numeric inputs                                              |
| Concurrency limits (§1.10.6)               | direct   | P1       | numeric inputs                                              |
| Recent Media Payload Messages (§1.10.6)    | direct   | P2       | numeric input                                               |
| Tool Result Inline Limit (§1.10.6)         | direct   | P2       | preset list                                                 |
| Shell Isolation Level (§1.10.6)            | direct   | P1       | choice list                                                 |
| Shell runtime bootstrap (§1.10.6)          | direct   | P2       | toggle                                                      |
| Linked git worktrees (§1.10.6)             | direct   | P2       | toggle                                                      |
| About / Check for Updates (§1.10.6)        | adapted  | P2       | check prints the result; install runs the updater and quits |
| Clear All Sessions & Workspace (§1.10.6)   | direct   | P2       | confirm prompt                                              |
| Reset All Data & Settings (§1.10.6)        | direct   | P2       | type-`RESET` confirm                                        |
| Inline Audio Attachments (§1.10.7)         | direct   | P2       | toggle                                                      |
| Tool-loop recovery legacy (§1.10.7)        | direct   | P2       | toggle                                                      |
| Max resample retries (§1.10.7)             | direct   | P2       | numeric input                                               |
| Dev tab (all rows) (§1.10.8)               | excluded | —        | dev-only build; developers use the GUI dev build            |
| Provider model list (§1.10.9)              | —        | —        | reference table; served through the P0 `/model` picker      |
| Thinking Depth provider mapping (§1.10.9)  | —        | —        | reference table; backend behaviour                          |
| Migration Back (§1.11)                     | direct   | P2       | leave the page                                              |
| Migration Export (§1.11)                   | adapted  | P2       | path argument plus sensitive-data prompt                    |
| Migration Import (§1.11)                   | adapted  | P2       | path argument                                               |
| Migration Password dialogs (§1.11)         | direct   | P2       | masked inputs                                               |
| Migration Progress (§1.11)                 | adapted  | P2       | single updating percent line                                |
| Migration Import preview (§1.11)           | direct   | P2       | text preview plus conflict choice                           |
| Migration Done (§1.11)                     | direct   | P2       | result table                                                |

### 2.14 Dev-only, hidden, and conditional (§1.12)

| Item (§1 ref)                               | Class    | Priority | Terminal form                                                   |
| ------------------------------------------- | -------- | -------- | --------------------------------------------------------------- |
| Settings Dev tab (§1.12)                    | excluded | —        | dev-only, as in the GUI                                         |
| Update auto-check (§1.12)                   | —        | —        | backend behaviour inherited by §2.2 Update toast                |
| `/history/search` (§1.12)                   | excluded | —        | hidden duplicate, as in the GUI                                 |
| App control bridge (§1.12)                  | excluded | —        | hidden automation hook, as in §2.2                              |
| Session API pairing dialog (§1.12)          | —        | —        | already classified P1 in §2.2 and §2.9                          |
| Slash commands and `@` mentions (§1.12)     | —        | —        | already classified in §2.6; discovery-by-typing stays identical |
| Agent Hub URL (§1.12)                       | excluded | —        | no rendered input in the GUI either; nothing to port            |
| Provider options without UI (§1.12)         | excluded | —        | no rendered control in the GUI; backend-only settings           |
| Message search (§1.12)                      | excluded | —        | no GUI screen calls it; out of scope for parity                 |
| Feature tips (§1.12)                        | excluded | —        | see §2.2 tips rows                                              |
| Starter templates, Morning Briefing (§1.12) | —        | —        | already classified in §2.12                                     |
| Onboarding banners (§1.12)                  | —        | —        | already classified in §2.3                                      |
| Isolation Settings in draft (§1.12)         | —        | —        | already classified in §2.3                                      |

### 2.15 Excluded rollup

All exclusions, with reasons, so a reader can audit that parity is the default:

1. Sidebar, sidebar toggle, Collapse sidebar — window chrome; commands replace navigation.
2. Theme toggle, Color Theme, Font Family, UI size — rendering concerns owned by the terminal emulator.
3. Feature tips (cards, links, CTAs) — GUI onboarding surface; the underlying "show tips" setting persists for the GUI.
4. App control bridge — hidden automation hook with no user surface.
5. `/history/search` route — hidden duplicate route, hidden in the GUI too.
6. Playbooks group headers — terminal lists are flat.
7. Knowledge view switcher — one terminal layout.
8. Dev tab — dev-only builds.
9. Agent Hub URL, provider options without UI, message search — no GUI surface exists to port.

### 2.16 P0 core (phase-1 release gate)

The P0 set is the full chat turn plus session entry, exactly as required:

- Send and stream: §2.3 composer and create-on-send, §2.6 input, §2.7 messages and rich content, §2.8 tool cards.
- Tool approval, askUser, secret prompt: §2.9.
- `/new`: §2.3 assistant picker, model and thinking, §2.11 New session.
- `/resume`: §2.11 Recent Sessions and Open from sidebar, §2.11 sidebar status dot.
- Reaching a first message: §2.3 AI model banner, §2.13 Default AI Model and Provider API Keys, §2.5 execution mode, §2.4 loading and error states, §2.5 workflow banner.

## 3. Commands and TUI screens

Added by #2751.

## 4. E2E scenarios

Added by #2752.
