import { safeInvoke } from './core';

// ========================================
// Session Management
// ========================================

/**
 * Remove a session including its workspace directory on the native side.
 * This calls the Tauri command `remove_session` implemented in the backend.
 * @param sessionId The ID of the session to remove
 */
export async function removeSession(sessionId: string): Promise<void> {
  return safeInvoke<void>('remove_session', { sessionId });
}

/**
 * Delete content store artifacts for a session (backend command).
 * This removes SQLite rows and search index directories for the given session.
 */
export async function deleteContentStore(sessionId: string): Promise<void> {
  return safeInvoke<void>('delete_content_store', { sessionId });
}

/**
 * Clear all agent sessions including data and workspaces (backend command).
 */
export async function clearAllSessions(): Promise<void> {
  return safeInvoke<void>('agent_clear_all_sessions');
}

/**
 * Factory reset the agent system (backend command).
 * Deletes all sessions, assistants, playbooks, mcp servers, and logs.
 */
export async function factoryReset(): Promise<void> {
  return safeInvoke<void>('agent_factory_reset');
}

/**
 * Switches to a specific session with optional async behavior.
 * @param sessionId The ID of the session to switch to
 * @param useAsync Whether to use async switching (default: true)
 * @returns A promise that resolves with session information
 */
export async function switchSession(
  sessionId: string,
  useAsync?: boolean,
): Promise<{
  success: boolean;
  message: string;
  session_id?: string;
  data?: unknown;
}> {
  return safeInvoke<{
    success: boolean;
    message: string;
    session_id?: string;
    data?: unknown;
  }>('switch_session', {
    request: { session_id: sessionId, use_async: useAsync },
  });
}
