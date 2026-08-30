export const DEFAULT_MAX_RECENT_MEDIA_MESSAGES = 1;
export const MAX_RECENT_MEDIA_MESSAGES = 5;

export function normalizeMaxRecentMediaMessages(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return DEFAULT_MAX_RECENT_MEDIA_MESSAGES;
  }

  return Math.min(
    MAX_RECENT_MEDIA_MESSAGES,
    Math.max(DEFAULT_MAX_RECENT_MEDIA_MESSAGES, Math.trunc(value)),
  );
}
