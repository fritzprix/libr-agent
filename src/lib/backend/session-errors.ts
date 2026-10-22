const SESSION_NOT_FOUND_PATTERN = /\bSession not found\b/;

/**
 * True when an agent session command failed because the session no longer
 * exists (deleted, or never existed). Callers must treat this as terminal and
 * stop re-opening / resuming that session.
 */
export function isSessionNotFoundError(error: unknown): boolean {
  const message =
    error instanceof Error
      ? error.message
      : typeof error === 'string'
        ? error
        : '';
  return SESSION_NOT_FOUND_PATTERN.test(message);
}
