import type { MessageLayoutStyle } from '@/lib/services/settings-service';

/** Readable column width for document-mode chat turns (matches draft composer rail). */
export const DOCUMENT_CONTENT_RAIL_CLASS = 'mx-auto w-full max-w-5xl';

export function isDocumentMessageLayout(
  layout: MessageLayoutStyle | undefined,
): boolean {
  return layout === 'document';
}
