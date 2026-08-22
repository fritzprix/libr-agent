/** Must match toast id used by compact-listener and session teardown cleanup. */
export function compactSessionToastId(sessionId: string): string {
  return `compact-${sessionId}`;
}
