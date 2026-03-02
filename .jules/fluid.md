# FLUID'S JOURNAL - THE PERFORMANCE LOG

## 2026-02-26 - MCP Server Management **Bottleneck:** Missing feedback and premature dialog closure during server deletion and active toggle. **Flow Restored:** Implemented `isDeleting` and `togglingStatus` states. Prevented `AlertDialog` closure until async operations complete. Added spinners to confirmation buttons and active toggle switch.

## 2026-02-26 - Settings Danger Zone **Bottleneck:** Delete All and Factory Reset dialogs closed immediately on click, leaving user unsure if action was proceeding. **Flow Restored:** Prevented dialog closure until async action completes. Added loading spinners to confirmation buttons.

## 2026-02-26 - Playbook Auto-Start **Bottleneck:** `AgentChatStartView` auto-started playbooks without disabling UI interactions, risking double submits. **Flow Restored:** Added `isCreating` true state during the async operation.

## 2026-02-26 - Drag and Drop File Attachments **Bottleneck:** Dropped files on `AgentDraftChatView` processed asynchronously without any loading indication. **Flow Restored:** Added `isAttachmentLoading` to UI to show "Uploading..." and disable input buttons.
