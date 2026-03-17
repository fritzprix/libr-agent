1. **Fix MCPServerDialog**:
   - In `src/features/mcp-servers/MCPServerDialog.tsx`, replace `onOpenChange={onCancel}` with `onOpenChange={(open) => !open && !isSaving && onCancel()}`.

2. **Fix MCPServerManagement**:
   - In `src/features/mcp-servers/MCPServerManagement.tsx`, I will check if any missing loading state is causing naked awaits or double submits. Actually, `MCPServerManagement.tsx` is already covered by the fluid journal memory: "Prevented `AlertDialog` closure until async operations complete. Added spinners to confirmation buttons and active toggle switch." I will double check `handleToggleActive` in `useMCPServerManagement.ts`.

3. **Check useMCPServerManagement.ts**:
   - Ensure `isDeleting` and `togglingStatus` states are correctly utilized.

4. **Verify other files according to Fluid's Pathological Jank Hunt**:
   - Check `Naked Awaits`
   - Check `Main-Thread Monsters`
   - Check `Layout Shifters`
