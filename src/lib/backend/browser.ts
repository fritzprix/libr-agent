import { safeInvoke } from './core';
import type { BrowserSession } from './types';

// ========================================
// Browser Session and Scripting Helpers
// Centralized wrappers for browser-related Tauri commands used by
// `BrowserToolProvider` and other browser features. These use `safeInvoke`
// so logging and error handling remain consistent across the app.
// ========================================

export interface BrowserProfileInfo {
  name: string;
  label: string;
  sourceLabel: string;
  sourceBrowser: string;
  importedAt: string;
  isDefault: boolean;
  /** chromium_user_data | firefox_cookies */
  importKind?: 'chromium_user_data' | 'firefox_cookies';
}

/** Installed browser Default profile available to import (no filesystem paths). */
export interface DiscoverableBrowserProfile {
  name: string;
  label: string;
  browserId: string;
  browserLabel: string;
}

export interface BrowserProfileImportReport {
  imported: string[];
  skipped: string[];
  warnings: string[];
  /** Friendly browser names still open among import targets (e.g. Chrome, Edge). */
  runningBrowsers?: string[];
}

export interface BrowserProfileImportReadiness {
  ready: boolean;
  runningBrowsers: string[];
}

export interface QuitBrowsersForImportReport {
  attempted: string[];
  stillRunning: string[];
  ready: boolean;
}

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

/** Lists imported browser profiles (names/labels only — no filesystem paths). */
export async function listBrowserProfiles(): Promise<BrowserProfileInfo[]> {
  return safeInvoke<BrowserProfileInfo[]>('list_browser_profiles');
}

/** Lists installed browser Default profiles available to import. */
export async function listDiscoverableBrowserProfiles(): Promise<
  DiscoverableBrowserProfile[]
> {
  return safeInvoke<DiscoverableBrowserProfile[]>(
    'list_discoverable_browser_profiles',
  );
}

/**
 * Whether cookie import can succeed for the selected profiles
 * (or all installed Defaults when omitted).
 */
export async function checkBrowserProfileImportReady(
  profileNames?: string[],
): Promise<BrowserProfileImportReadiness> {
  return safeInvoke<BrowserProfileImportReadiness>(
    'check_browser_profile_import_ready',
    {
      profileNames:
        profileNames && profileNames.length > 0 ? profileNames : null,
    },
  );
}

/**
 * Quit locking browsers after explicit user consent.
 * When `profileNames` is set, only quit browsers needed for those profiles.
 * Only allowlisted browser processes are targeted (never WebView2).
 */
export async function quitBrowsersForProfileImport(
  userConfirmed: boolean,
  profileNames?: string[],
): Promise<QuitBrowsersForImportReport> {
  return safeInvoke<QuitBrowsersForImportReport>(
    'quit_browsers_for_profile_import',
    {
      userConfirmed,
      profileNames:
        profileNames && profileNames.length > 0 ? profileNames : null,
    },
  );
}

/**
 * Import selected browser profiles into app-local storage.
 * When `profileNames` is omitted, all discoverable Default profiles are tried.
 * `preferredDefault` marks that slug as the agent `use_profile` primary when imported.
 */
export async function importBrowserProfiles(options?: {
  profileNames?: string[];
  preferredDefault?: string;
}): Promise<BrowserProfileImportReport> {
  const profileNames = options?.profileNames;
  return safeInvoke<BrowserProfileImportReport>('import_browser_profiles', {
    profileNames:
      profileNames && profileNames.length > 0 ? profileNames : null,
    preferredDefault: options?.preferredDefault ?? null,
  });
}

/** Choose which imported profile agents use with `use_profile: true`. */
export async function setDefaultBrowserProfile(name: string): Promise<void> {
  return safeInvoke<void>('set_default_browser_profile', { name });
}

/** Remove an imported browser profile and delete its app-local copy. */
export async function removeBrowserProfile(name: string): Promise<void> {
  return safeInvoke<void>('remove_browser_profile', { name });
}

/**
 * Open the imported Chromium profile in system Chrome for manual sign-in.
 * No CDP — required for Google account login.
 */
export async function openBrowserProfileForSignIn(name: string): Promise<void> {
  return safeInvoke<void>('open_browser_profile_for_signin', { name });
}
