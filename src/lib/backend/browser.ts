import { safeInvoke } from './core';
import type { BrowserSession } from './types';

// ========================================
// Browser Session and Scripting Helpers
// Centralized wrappers for browser-related Tauri commands used by
// `BrowserToolProvider` and other browser features. These use `safeInvoke`
// so logging and error handling remain consistent across the app.
// ========================================

/**
 * Creates a new browser session controlled by the backend.
 * @param params The parameters for the new session, including the initial URL.
 * @param params.url The initial URL to open.
 * @param params.title An optional title for the session.
 * @returns A promise that resolves to the unique ID of the new session.
 */
export async function createBrowserSession(params: {
  url: string;
  title?: string | null;
}): Promise<{ session_id: string; message: string }> {
  return safeInvoke<{ session_id: string; message: string }>(
    'create_browser_session',
    params,
  );
}

/**
 * Closes an active browser session.
 * @param sessionId The ID of the session to close.
 * @returns A promise that resolves when the session is closed.
 */
export async function closeBrowserSession(sessionId: string): Promise<void> {
  return safeInvoke<void>('close_browser_session', { sessionId });
}

/**
 * Lists all active browser sessions.
 * @returns A promise that resolves to an array of `BrowserSession` objects.
 */
export async function listBrowserSessions(): Promise<BrowserSession[]> {
  return safeInvoke<BrowserSession[]>('list_browser_sessions');
}

/**
 * Navigates a browser session to a new URL.
 * @param sessionId The ID of the browser session.
 * @param url The URL to navigate to.
 * @returns A promise that resolves with the result of the navigation.
 */
export async function navigateToUrl(
  sessionId: string,
  url: string,
): Promise<string> {
  return safeInvoke<string>('navigate_to_url', { sessionId, url });
}
