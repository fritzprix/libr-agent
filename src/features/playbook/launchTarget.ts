import type { Playbook } from '@/types/playbook';

export type PlaybookLaunchDecision =
  | { kind: 'pinned'; sessionId: string }
  | { kind: 'new'; reason: 'unpinned' | 'session-missing' };

export function getPinnedLaunchSessionId(
  playbook: Pick<Playbook, 'defaultTargetSession'>,
): string | null {
  const target = playbook.defaultTargetSession;
  if (target?.mode !== 'pin') {
    return null;
  }
  const sessionId = target.sessionId?.trim();
  return sessionId ? sessionId : null;
}

/**
 * Resolve which session Playbook Start should open.
 * Pin mode with a live session → reuse it; otherwise fall back to new session.
 *
 * `resolveSession` must return the canonical storage id (backend resolves
 * legacy short refs) or null when missing.
 */
export async function resolvePlaybookLaunchTarget(
  playbook: Pick<Playbook, 'defaultTargetSession'>,
  resolveSession: (sessionRef: string) => Promise<string | null>,
): Promise<PlaybookLaunchDecision> {
  const pinnedRef = getPinnedLaunchSessionId(playbook);
  if (!pinnedRef) {
    return { kind: 'new', reason: 'unpinned' };
  }

  const sessionId = await resolveSession(pinnedRef);
  if (!sessionId) {
    return { kind: 'new', reason: 'session-missing' };
  }

  return { kind: 'pinned', sessionId };
}
