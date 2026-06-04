# Frontend Deduplication

When hooking into `agent_resume_session` to re-emit pending approvals, the frontend may receive the `ToolExecutionRequiresApproval` event multiple times for the same tool call if the user navigates back and forth or if the resume logic is called multiple times.

To prevent the UI from showing duplicate "Approve/Reject" buttons for the same tool execution, we must deduplicate the state update.

## Update `AgentSessionContext.tsx`

In `src/context/AgentSessionContext.tsx`, locate the event listener for `agent:event` and the specific `case 'toolExecutionRequiresApproval':`.

Modify the `setPendingApprovals` call:

```tsx
// Replace this:
// setPendingApprovals((prev) => [
//   ...prev,
//   {
//     toolCallId: payload.toolCallId,
//     toolName: payload.toolName,
//     arguments: payload.arguments,
//   },
// ]);

// With this:
setPendingApprovals((prev) => {
  // Prevent duplicate entries on session resume
  if (prev.some((p) => p.toolCallId === payload.toolCallId)) {
    return prev;
  }
  return [
    ...prev,
    {
      toolCallId: payload.toolCallId,
      toolName: payload.toolName,
      arguments: payload.arguments,
    },
  ];
});
```

This ensures the `PendingApprovalWidget` correctly reflects the unique tool calls waiting for response without duplicating elements on the screen.
