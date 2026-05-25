import type { Message } from '@/models/chat';

export function isCompactSummaryMessage(
  message: Pick<Message, 'id' | 'source'> | undefined,
): boolean {
  if (!message) {
    return false;
  }

  return (
    message.source === 'compact-summary' ||
    message.id.startsWith('compact-summary-')
  );
}
