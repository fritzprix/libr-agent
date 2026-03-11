# FLUID'S JOURNAL - THE PERFORMANCE LOG

## 2026-02-26 - MCP Server Management **Bottleneck:** Missing feedback and premature dialog closure during server deletion and active toggle. **Flow Restored:** Implemented `isDeleting` and `togglingStatus` states. Prevented `AlertDialog` closure until async operations complete. Added spinners to confirmation buttons and active toggle switch.

## 2026-02-26 - Settings Danger Zone **Bottleneck:** Delete All and Factory Reset dialogs closed immediately on click, leaving user unsure if action was proceeding. **Flow Restored:** Prevented dialog closure until async action completes. Added loading spinners to confirmation buttons.

## 2026-02-26 - Playbook Auto-Start **Bottleneck:** `AgentChatStartView` auto-started playbooks without disabling UI interactions, risking double submits. **Flow Restored:** Added `isCreating` true state during the async operation.

## 2026-02-26 - Drag and Drop File Attachments **Bottleneck:** Dropped files on `AgentDraftChatView` processed asynchronously without any loading indication. **Flow Restored:** Added `isAttachmentLoading` to UI to show "Uploading..." and disable input buttons.

## 2026-02-26 - Assistant Editor Save **Bottleneck:** Assistant dialog closed immediately on click before `commit` finished, and Save button didn't disable during save. **Flow Restored:** Added `await commit()` to `handleSave` and utilized `isLoading` state to disable buttons and prevent premature closure.

## 2026-03-03 - [DangerZoneSettings, SkillsEditor, MCPServerManagement] **Bottleneck:** [Naked async onClick handlers lacking proper void return or UI state isolation] **Flow Restored:** [Eradicated naked async handlers by refactoring to use proper void functions and chaining for promise resolution]

## 2026-03-03 - [AgentWorkspacePanel, GeneralTab] **Bottleneck:** [Naked await causing potential duplicate native window invocations on double-click] **Flow Restored:** [Added loading UI state for Open in Explorer and Open in Terminal invocations]

## 2024-05-18 - [GeneralTab/handleBrowseEvents] **Bottleneck:** [Missing loading state on directory browse native OS dialog causing double submits] **Flow Restored:** [Added isBrowsing state, disabled button, and spinner]

## 2024-05-18 - [AgentWorkspacePanel/handleBrowseFolder] **Bottleneck:** [Missing loading state on directory browse native OS dialog causing double submits] **Flow Restored:** [Added isBrowsing state, disabled button, and spinner]

## 2024-05-24 - [ScheduledTasksPage] **Bottleneck:** Naked awaits on task toggle and delete operations caused missing feedback and possible double-clicks. **Flow Restored:** Added Set-based tracking states for toggling/deleting and attached `disabled` and visual spinners (`Clock className="animate-spin"`) to respective UI elements.
## 2026-03-03 - [SessionHistoryPanel] **Bottleneck:** [Main-Thread Block: filtering large array of 10,000+ items directly inside the render cycle blocking typing] **Flow Restored:** [Applied `useDeferredValue` to search queries and session list to unblock text input and applied transparency during filtering transition]
