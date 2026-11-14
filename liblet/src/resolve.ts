/**
 * Session resolution helpers for `--continue` (gatekeeper G1.2 on #2741).
 * Pure and platform-injectable so the Windows path lower-casing is testable
 * on any machine.
 */

import * as path from 'node:path';

import type { SessionMetadata } from './types.js';

/**
 * Canonical form of a workspace path for matching. On Windows the backend
 * can hand back a differently-cased path, so comparison is case-insensitive
 * there; trailing separators are ignored everywhere.
 */
export function normalizeWorkspace(
  p: string,
  platform: NodeJS.Platform = process.platform,
): string {
  let out = path.resolve(p).replace(/[\\/]+$/, '');
  if (platform === 'win32') out = out.toLowerCase();
  return out;
}

/** The session whose workspace matches `cwd`, if any. */
export function findContinueSession(
  sessions: SessionMetadata[],
  cwd: string,
  platform: NodeJS.Platform = process.platform,
): SessionMetadata | undefined {
  const target = normalizeWorkspace(cwd, platform);
  return sessions.find(
    (s) =>
      Boolean(s.workspaceOverride) &&
      normalizeWorkspace(s.workspaceOverride as string, platform) === target,
  );
}
